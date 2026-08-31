use std::{any::Any, collections::HashMap, fmt, sync::Arc};

use argusflow_core::{ResourceId, ResourceRef, ResourceTypeId};
use async_trait::async_trait;

use crate::{
    error::RuntimeError,
    execution::{AccessSet, ResourceAccess, ResourceAccessKey, ResourceAccessMode},
};

/// 一个资源类型的异步回收策略。
///
/// 新资源提供器在注册自己的节点时同时提供该策略；`ResourceTable` 因而无需为每种
/// 资源增加枚举分支。实现必须验证传入值的具体类型，类型不匹配属于运行时不变量错误。
#[async_trait]
pub trait ResourceCleanup: Send + Sync {
    /// 回收一个已绑定的类型擦除资源实例。
    async fn cleanup(&self, value: &(dyn Any + Send + Sync)) -> Result<(), RuntimeError>;
}

/// 资源边界内的类型擦除实例；热路径获取后仍使用具体强类型引用。
#[derive(Clone)]
pub struct ResourceEntry {
    /// 单次运行内的稳定资源标识。
    id: ResourceId,
    /// 注册表和端口校验共享的稳定资源类型。
    kind: ResourceTypeId,
    /// 只在资源表边界擦除；调用方通过 `ResourceTable::get` 恢复具体类型。
    value: Arc<dyn Any + Send + Sync>,
    /// 与实例一同冻结的回收策略。
    cleanup: Arc<dyn ResourceCleanup>,
    /// Scheduler 对该真实外部对象使用的稳定冲突键。
    access_key: ResourceAccessKey,
}

impl fmt::Debug for ResourceEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceEntry")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("access_key", &self.access_key)
            .finish_non_exhaustive()
    }
}

/// 单次运行独占的真实资源与逻辑引用绑定表。
#[derive(Debug, Default)]
pub struct ResourceTable {
    /// 按运行时 ID 保存真实资源，避免把 OS 身份放进工作流 JSON。
    resources: HashMap<ResourceId, ResourceEntry>,
    /// 将生产节点输出端口绑定到运行时资源 ID。
    bindings: HashMap<ResourceRef, ResourceId>,
    /// 按获取顺序记录资源，工作流结束时反向清理。
    acquisition_order: Vec<ResourceId>,
}

impl ResourceTable {
    /// 绑定一个强类型资源实例及其回收策略。
    pub fn insert<T>(
        &mut self,
        reference: ResourceRef,
        resource_id: ResourceId,
        kind: ResourceTypeId,
        value: T,
        cleanup: Arc<dyn ResourceCleanup>,
        access_key: ResourceAccessKey,
    ) where
        T: Any + Send + Sync,
    {
        self.resources.insert(
            resource_id,
            ResourceEntry {
                id: resource_id,
                kind,
                value: Arc::new(value),
                cleanup,
                access_key,
            },
        );
        self.bindings.insert(reference, resource_id);
        self.acquisition_order.push(resource_id);
    }

    /// 解析逻辑引用对应的调度资源键。
    pub fn access_key(&self, reference: &ResourceRef) -> Result<ResourceAccessKey, RuntimeError> {
        let resource_id =
            self.bindings
                .get(reference)
                .ok_or_else(|| RuntimeError::ResourceUnavailable {
                    reference: reference.clone(),
                })?;
        self.resources
            .get(resource_id)
            .map(|entry| entry.access_key.clone())
            .ok_or_else(|| RuntimeError::ResourceUnavailable {
                reference: reference.clone(),
            })
    }

    /// 返回清理阶段需要独占的全部真实外部资源。
    pub(crate) fn cleanup_access_set(&self) -> AccessSet {
        AccessSet {
            resources: self
                .resources
                .values()
                .map(|entry| ResourceAccess {
                    key: entry.access_key.clone(),
                    mode: ResourceAccessMode::Exclusive,
                })
                .collect(),
        }
    }

    /// 解析逻辑引用，并在资源边界恢复调用方要求的具体强类型。
    pub fn get<T>(&self, reference: &ResourceRef, kind: &ResourceTypeId) -> Result<&T, RuntimeError>
    where
        T: Any + Send + Sync,
    {
        let resource_id =
            self.bindings
                .get(reference)
                .ok_or_else(|| RuntimeError::ResourceUnavailable {
                    reference: reference.clone(),
                })?;
        self.resources
            .get(resource_id)
            .filter(|entry| &entry.kind == kind)
            .and_then(|entry| entry.value.downcast_ref::<T>())
            .ok_or_else(|| RuntimeError::ResourceUnavailable {
                reference: reference.clone(),
            })
    }

    /// 按获取顺序的逆序回收所有资源，不依赖中央资源枚举。
    pub(crate) async fn cleanup_all(&self) -> Result<(), RuntimeError> {
        let mut first_error = None;
        for resource_id in self.acquisition_order.iter().rev() {
            if let Some(resource) = self.resources.get(resource_id)
                && let Err(error) = resource.cleanup.cleanup(resource.value.as_ref()).await
                && first_error.is_none()
            {
                // 记录第一项失败，但继续释放其它独立资源，避免一个插件清理器阻断全局回收。
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        any::Any,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use argusflow_core::{ResourceId, ResourceRef, ResourceTypeId};
    use async_trait::async_trait;

    use crate::{ResourceAccessKey, ResourceCleanup, ResourceTable, RuntimeError};

    /// 记录调用次数，并按配置模拟单个插件清理失败。
    struct CountingCleanup {
        calls: Arc<AtomicUsize>,
        fails: bool,
    }

    #[async_trait]
    impl ResourceCleanup for CountingCleanup {
        async fn cleanup(&self, _value: &(dyn Any + Send + Sync)) -> Result<(), RuntimeError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fails {
                Err(RuntimeError::NodeExecution {
                    message: "test cleanup failed".to_owned(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn cleanup_failure_does_not_skip_other_registered_resources() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut resources = ResourceTable::default();
        for (index, fails) in [false, true, false].into_iter().enumerate() {
            let resource_id = ResourceId::new();
            resources.insert(
                ResourceRef {
                    producer_node_id: format!("resource-{index}"),
                    output_name: "value".to_owned(),
                },
                resource_id,
                ResourceTypeId::new("test.resource"),
                index,
                Arc::new(CountingCleanup {
                    calls: Arc::clone(&calls),
                    fails,
                }),
                ResourceAccessKey::Runtime(resource_id),
            );
        }

        let error = resources
            .cleanup_all()
            .await
            .expect_err("the first cleanup failure should be reported after all attempts");

        assert!(matches!(error, RuntimeError::NodeExecution { .. }));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }
}
