//! 应用资源拥有自有进程，窗口资源仅持有身份租约。
use argusflow_core::Operation;
use argusflow_runtime::{Resource, RunError, TaskFuture};
use argusflow_windows::{Application, WindowIdentity};
use std::any::Any;

pub(crate) struct ApplicationResource(pub Application);
impl Resource for ApplicationResource {
    fn resource_type(&self) -> &str {
        super::APPLICATION
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn cleanup<'a>(&'a self, operation: &'a Operation) -> TaskFuture<'a, ()> {
        Box::pin(async move {
            self.0
                .shutdown(operation)
                .await
                .map_err(|e| RunError::from(argusflow_core::Failure::from(e)))
        })
    }
}
pub(crate) struct WindowResource(pub WindowIdentity);
impl Resource for WindowResource {
    fn resource_type(&self) -> &str {
        super::WINDOW
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn cleanup<'a>(&'a self, _operation: &'a Operation) -> TaskFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}
