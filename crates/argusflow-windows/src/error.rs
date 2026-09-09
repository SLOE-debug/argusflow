//! 本能力的错误类型，公共分类和诊断数据来自 core。
use argusflow_core::{Failure, FailureKind};
use std::{error::Error, fmt, ops::Deref};

/// windows 能力错误；source 保留原始原因，默认日志仅输出分类和阶段。
#[derive(Debug, Clone)]
pub struct WindowsError(Failure);
impl WindowsError {
    pub(crate) fn new(kind: FailureKind, stage: &'static str, message: impl Into<String>) -> Self {
        Self(Failure::new(kind, stage, message))
    }
    pub(crate) fn with_source(self, source: impl Error + Send + Sync + 'static) -> Self {
        Self(self.0.with_source(source))
    }
    pub(crate) fn with_resource(self, resource: impl Into<String>) -> Self {
        Self(self.0.with_resource(resource))
    }
}
impl Deref for WindowsError {
    type Target = Failure;
    fn deref(&self) -> &Failure {
        &self.0
    }
}
impl From<Failure> for WindowsError {
    fn from(failure: Failure) -> Self {
        Self(failure)
    }
}
impl From<WindowsError> for Failure {
    fn from(error: WindowsError) -> Self {
        error.0
    }
}
impl fmt::Display for WindowsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for WindowsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}
