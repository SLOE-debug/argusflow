//! 宿主注入共享服务；工作流资源节点不会创建重复的底层服务。
use argusflow_automation::QuerySource;
use argusflow_core::Operation;
use argusflow_runtime::{Resource, TaskFuture};
use std::{any::Any, collections::BTreeMap};

/// 已由宿主初始化的自动化服务和可借用来源。
#[derive(Clone, Default)]
pub struct AutomationHost {
    /// 可按显式名称绑定的 UIA、DOM、OCR 来源。
    pub sources: BTreeMap<String, QuerySource>,
    /// 供新附加窗口复用的 UIA 服务。
    #[cfg(windows)]
    pub uia: Option<argusflow_windows::UiaRuntime>,
    /// 供窗口操作复用的真实输入服务。
    #[cfg(windows)]
    pub input: Option<argusflow_windows::InputService>,
}
/// 一个宿主或作用域绑定的查询来源；释放本包装不关闭共享服务。
pub struct QuerySourceResource(pub(crate) QuerySource);
impl QuerySourceResource {
    /// 把显式平台来源包装为运行资源，所有权仍由宿主或父资源持有。
    pub fn new(source: QuerySource) -> Self {
        Self(source)
    }
}
impl Resource for QuerySourceResource {
    fn resource_type(&self) -> &str {
        super::SOURCE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn cleanup<'a>(&'a self, _operation: &'a Operation) -> TaskFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}
