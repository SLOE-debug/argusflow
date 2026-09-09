//! 本能力的错误类型，公共分类和诊断数据来自 core。
use argusflow_core::{Failure, FailureKind};
use std::{error::Error, fmt, ops::Deref};

/// browser 能力错误；source 保留原始原因，默认日志仅输出分类和阶段。
#[derive(Debug, Clone)]
pub struct BrowserError(Failure);
impl BrowserError {
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
impl Deref for BrowserError {
    type Target = Failure;
    fn deref(&self) -> &Failure {
        &self.0
    }
}
impl From<Failure> for BrowserError {
    fn from(failure: Failure) -> Self {
        Self(failure)
    }
}
impl From<BrowserError> for Failure {
    fn from(error: BrowserError) -> Self {
        error.0
    }
}
impl fmt::Display for BrowserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for BrowserError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}
