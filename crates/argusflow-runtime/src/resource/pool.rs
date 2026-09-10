//! 回收失败的资源继续占据额度，由引擎保留直到显式重试成功。
use crate::{Resource, RunError};
use argusflow_core::{Operation, OperationOptions};
use argusflow_workflow::ErrorKind;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

#[derive(Clone)]
pub(crate) struct Lease {
    pub id: Option<u64>,
    pub resource: Arc<dyn Resource>,
}
struct Entry {
    run: u64,
    resource: Arc<dyn Resource>,
    dependencies: BTreeSet<u64>,
    _permit: Option<OwnedSemaphorePermit>,
}
pub(crate) struct ResourcePool {
    entries: Mutex<BTreeMap<u64, Entry>>,
    cleanup: Mutex<()>,
    capacity: Arc<Semaphore>,
    next: AtomicU64,
}
impl ResourcePool {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
            cleanup: Mutex::new(()),
            capacity: Arc::new(Semaphore::new(capacity)),
            next: AtomicU64::new(1),
        }
    }
    pub fn reserve(&self, count: usize) -> Result<Option<OwnedSemaphorePermit>, RunError> {
        if count == 0 {
            return Ok(None);
        }
        let count =
            u32::try_from(count).map_err(|_| RunError::new(ErrorKind::Limit, "资源端口过多"))?;
        self.capacity
            .clone()
            .try_acquire_many_owned(count)
            .map(Some)
            .map_err(|_| RunError::new(ErrorKind::Busy, "资源额度已满，包含等待清理的对象"))
    }
    pub async fn adopt(
        &self,
        run: u64,
        resources: BTreeMap<String, Arc<dyn Resource>>,
        dependencies: BTreeSet<u64>,
        mut permit: Option<OwnedSemaphorePermit>,
    ) -> BTreeMap<String, Lease> {
        let mut entries = self.entries.lock().await;
        resources
            .into_iter()
            .map(|(port, resource)| {
                let id = self.next.fetch_add(1, Ordering::Relaxed);
                let retained = permit.as_mut().and_then(|permit| permit.split(1));
                entries.insert(
                    id,
                    Entry {
                        run,
                        resource: resource.clone(),
                        dependencies: dependencies.clone(),
                        _permit: retained,
                    },
                );
                (
                    port,
                    Lease {
                        id: Some(id),
                        resource,
                    },
                )
            })
            .collect()
    }
    pub async fn count(&self) -> usize {
        self.entries.lock().await.len()
    }
    pub async fn close(&self, id: u64, operation: &Operation) -> Result<(), RunError> {
        let _exclusive = tokio::time::timeout(operation.remaining(), self.cleanup.lock())
            .await
            .map_err(|_| RunError::new(ErrorKind::Cleanup, "等待资源清理锁超时"))?;
        let resource = {
            let entries = self.entries.lock().await;
            if entries
                .values()
                .any(|entry| entry.dependencies.contains(&id))
            {
                return Err(RunError::new(
                    ErrorKind::Cleanup,
                    "依赖资源尚未释放，保留父资源",
                ));
            }
            let Some(entry) = entries.get(&id) else {
                return Ok(());
            };
            entry.resource.clone()
        };
        let result = tokio::time::timeout(
            operation.remaining(),
            crate::execution::guard_future(async { resource.cleanup(operation).await }),
        )
        .await;
        match result {
            Ok(Ok(())) => {
                self.entries.lock().await.remove(&id);
                Ok(())
            }
            Ok(Err(mut error)) => {
                error.kind = ErrorKind::Cleanup;
                Err(error)
            }
            Err(_) => {
                operation.cancel();
                Err(RunError::new(
                    ErrorKind::Cleanup,
                    "资源清理超时，仍保留资源及占用",
                ))
            }
        }
    }
    pub async fn cleanup_run(&self, run: Option<u64>, timeout: Duration) -> Vec<RunError> {
        let ids = self
            .entries
            .lock()
            .await
            .iter()
            .filter(|(_, e)| run.is_none_or(|run| e.run == run))
            .map(|(id, _)| *id)
            .rev()
            .collect::<Vec<_>>();
        let operation = match OperationOptions::new(timeout) {
            Ok(options) => Operation::new(options),
            Err(error) => return vec![error.into()],
        };
        let mut errors = Vec::new();
        for id in ids {
            if let Err(error) = self.close(id, &operation).await {
                errors.push(error);
            }
        }
        errors
    }
}
