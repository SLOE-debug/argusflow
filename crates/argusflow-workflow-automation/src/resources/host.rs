//! 宿主注入共享服务；工作流资源节点不会创建重复的底层服务。
use argusflow_automation::QuerySource;
use argusflow_core::Operation;
use argusflow_runtime::{Resource, RunError, TaskFuture};
use std::{any::Any, collections::BTreeMap, sync::Arc};

/// 已由宿主初始化的自动化服务和可借用来源。
#[derive(Clone, Default)]
pub struct AutomationHost {
    /// 惰性初始化并由宿主统一关闭的共享 OCR 服务。
    #[cfg(windows)]
    pub ocr: super::OcrServices,
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
pub struct QuerySourceResource(Arc<dyn QuerySourceProvider>);
/// 查询范围的统一绑定与生命周期契约；动态范围在每次查询前重新解析。
pub trait QuerySourceProvider: Send + Sync {
    /// 获取当前有效的查询来源。
    fn resolve(&self) -> Result<QuerySource, RunError>;
    /// 释放本绑定拥有的资源；借用来源不关闭宿主服务。
    fn cleanup<'a>(&'a self, operation: &'a Operation) -> TaskFuture<'a, ()>;
}
struct StaticSource(QuerySource);
impl QuerySourceProvider for StaticSource {
    fn resolve(&self) -> Result<QuerySource, RunError> {
        Ok(self.0.clone())
    }
    fn cleanup<'a>(&'a self, _: &'a Operation) -> TaskFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}
impl QuerySourceResource {
    /// 把显式平台来源包装为运行资源，所有权仍由宿主或父资源持有。
    pub fn new(source: QuerySource) -> Self {
        Self::provider(Arc::new(StaticSource(source)))
    }
    /// 包装拥有动态范围或独立生命周期的来源提供者。
    pub fn provider(provider: Arc<dyn QuerySourceProvider>) -> Self {
        Self(provider)
    }
    /// 每次使用前解析最新范围。
    pub fn resolve(&self) -> Result<QuerySource, RunError> {
        self.0.resolve()
    }
}
impl Resource for QuerySourceResource {
    fn resource_type(&self) -> &str {
        super::SOURCE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn cleanup<'a>(&'a self, operation: &'a Operation) -> TaskFuture<'a, ()> {
        self.0.cleanup(operation)
    }
}
