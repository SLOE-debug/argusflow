//! 自建浏览器的进程、子进程与临时目录所有权。
use crate::BrowserError as Failure;
use argusflow_core::{FailureKind, Operation};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{
    process::Child,
    sync::{OwnedSemaphorePermit, Semaphore},
};

static PROCESS_SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
pub(crate) fn reserve() -> Result<OwnedSemaphorePermit, Failure> {
    PROCESS_SLOTS
        .get_or_init(|| Arc::new(Semaphore::new(8)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            Failure::new(
                FailureKind::Busy,
                "browser_launch",
                "自建浏览器或尚未完成的清理已达 8 个",
            )
        })
}
pub(crate) struct Managed {
    pub(crate) child: Option<Child>,
    pub(crate) profile: Option<tempfile::TempDir>,
    pub(crate) permit: Option<OwnedSemaphorePermit>,
    #[cfg(windows)]
    job: Option<super::job::Job>,
}
impl Managed {
    pub(crate) fn new(
        child: Child,
        profile: tempfile::TempDir,
        permit: OwnedSemaphorePermit,
    ) -> Result<Self, Failure> {
        let mut managed = Self {
            child: Some(child),
            profile: Some(profile),
            permit: Some(permit),
            #[cfg(windows)]
            job: None,
        };
        #[cfg(windows)]
        {
            managed.job = Some(super::job::Job::assign(
                managed.child.as_ref().expect("owned child"),
            )?);
        }
        Ok(managed)
    }
    pub(crate) async fn reap(&mut self, operation: &Operation) -> Result<(), Failure> {
        #[cfg(windows)]
        drop(self.job.take()); // KILL_ON_JOB_CLOSE 同时终止自有子进程。
        if let Some(child) = &mut self.child {
            child
                .start_kill()
                .map_err(|error| failure("无法终止自建浏览器").with_source(error))?;
            tokio::time::timeout(operation.remaining(), child.wait())
                .await
                .map_err(|_| {
                    Failure::new(
                        FailureKind::Timeout,
                        "browser_shutdown",
                        "自建浏览器尚未退出",
                    )
                })?
                .map_err(|error| failure("回收自建浏览器失败").with_source(error))?;
            self.child.take();
        }
        if let Some(profile) = self.profile.as_ref() {
            // Windows 子进程退出时文件句柄释放略有延迟；只重试目录清理。
            loop {
                match tokio::time::timeout(
                    operation.remaining(),
                    tokio::fs::remove_dir_all(profile.path()),
                )
                .await
                .map_err(|_| {
                    Failure::new(FailureKind::Timeout, "browser_cleanup", "临时目录清理超时")
                })? {
                    Ok(()) => break,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                    Err(error) if operation.remaining() <= Duration::from_millis(25) => {
                        return Err(failure("临时目录仍被占用").with_source(error));
                    }
                    Err(_) => tokio::time::sleep(Duration::from_millis(25)).await,
                }
            }
        }
        self.profile.take();
        self.permit.take();
        Ok(())
    }
}
impl Drop for Managed {
    fn drop(&mut self) {
        if self.child.is_none() && self.profile.is_none() {
            return;
        }
        #[cfg(windows)]
        drop(self.job.take());
        if let Some(child) = &mut self.child {
            let _ = child.start_kill();
        }
        let mut child = self.child.take();
        let profile = self.profile.take();
        let permit = self.permit.take();
        // 取消 launch 或直接 Drop 也必须等待进程退出；不提前删除使用中的 profile。
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            if let Some(profile) = profile {
                let path = profile.keep();
                tracing::warn!(path=?path,"runtime gone; browser profile retained after termination request");
            }
            return;
        };
        runtime.spawn(async move {
            let _permit = permit;
            if let Some(child) = &mut child {
                match tokio::time::timeout(Duration::from_secs(10), child.wait()).await {
                    Ok(Ok(_)) => {}
                    _ => {
                        if let Some(profile) = profile {
                            let path = profile.keep();
                            tracing::warn!(path=?path,"browser did not exit; profile retained");
                        }
                        return;
                    }
                }
            }
            if let Some(profile) = profile {
                let path = profile.keep();
                if let Err(error) = tokio::fs::remove_dir_all(&path).await {
                    tracing::warn!(path=?path,error=%error,"browser profile cleanup failed");
                }
            }
        });
    }
}
fn failure(message: &str) -> Failure {
    Failure::new(FailureKind::Unavailable, "browser_cleanup", message)
}
