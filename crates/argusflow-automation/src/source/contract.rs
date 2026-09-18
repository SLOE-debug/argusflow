//! 三个真实来源共享查询和动作边界，测试替身不进入生产实现。
use argusflow_aql::BoundQuery;
use argusflow_core::{Failure, Operation};
use std::{future::Future, pin::Pin};

pub(crate) type SourceFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, Failure>> + Send + 'a>>;
pub(crate) enum FocusedInput<'a> {
    Text(&'a str),
    Keys(&'a [argusflow_core::Key]),
}
pub(crate) trait SourceBackend: Send + Sync {
    type Target: Send + Sync;
    fn preview<'a>(
        &'a self,
        query: &'a BoundQuery,
        operation: &'a Operation,
    ) -> SourceFuture<'a, Vec<argusflow_aql::SpatialPreview>>;
    fn focused_input<'a>(
        &'a self,
        _target: &'a Self::Target,
        _input: FocusedInput<'a>,
        _operation: &'a Operation,
    ) -> SourceFuture<'a, ()> {
        Box::pin(async {
            Err(Failure::new(
                argusflow_core::FailureKind::Unsupported,
                "aql_focused_input",
                "此来源不支持已聚焦目标输入",
            ))
        })
    }
    fn find<'a>(
        &'a self,
        query: &'a BoundQuery,
        operation: &'a Operation,
    ) -> SourceFuture<'a, Vec<Self::Target>>;
    fn click<'a>(
        &'a self,
        target: &'a Self::Target,
        operation: &'a Operation,
    ) -> SourceFuture<'a, ()>;
    fn type_text<'a>(
        &'a self,
        target: &'a Self::Target,
        text: &'a str,
        operation: &'a Operation,
    ) -> SourceFuture<'a, ()>;
}
