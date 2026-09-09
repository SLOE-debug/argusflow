//! 有界补偿任务；额度在副作用之前取得，直到清理完成才释放。
use super::connection::Connection;
use argusflow_core::{Operation, OperationOptions};
use serde_json::Value;
use tokio::sync::OwnedSemaphorePermit;

/// 未取得新资源身份时，断开连接让 disposeOnDetach 和 session 自动回收生效。
pub(crate) struct UnknownResource(pub(crate) Option<Connection>, pub(crate) Operation);
impl Drop for UnknownResource {
    fn drop(&mut self) {
        if self.1.effect() == argusflow_core::Effect::Unconfirmed
            && let Some(connection) = &self.0
        {
            let _ = connection.inner.stop.send(true);
        }
    }
}

pub(crate) struct Cleanup {
    pub(crate) connection: Connection,
    pub(crate) session: Option<String>,
    pub(crate) commands: Vec<(&'static str, Value)>,
    pub(crate) permit: Option<OwnedSemaphorePermit>,
}
impl Cleanup {
    pub(crate) fn new(
        connection: Connection,
        session: Option<String>,
        permit: OwnedSemaphorePermit,
    ) -> Self {
        Self {
            connection,
            session,
            commands: Vec::new(),
            permit: Some(permit),
        }
    }
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        if self.commands.is_empty() {
            return;
        }
        let Some(permit) = self.permit.take() else {
            return;
        };
        let connection = self.connection.clone();
        let session = self.session.clone();
        let commands = std::mem::take(&mut self.commands);
        // 最多一个输入补偿任务和 max_in_flight 个对象/会话清理任务。
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            let _ = connection.inner.stop.send(true);
            return;
        };
        runtime.spawn(async move {
            let _permit = permit;
            let operation = Operation::new(OperationOptions::default());
            let _cancel = operation.cancel_on_drop();
            for (method, params) in commands.into_iter().rev() {
                if let Err(error) = connection
                    .command_wait(session.as_deref(), method, params, &operation)
                    .await
                {
                    if matches!(error.kind(),argusflow_core::FailureKind::StaleHandle | argusflow_core::FailureKind::Unavailable | argusflow_core::FailureKind::Closed) {
                        // 文档/session 或整个连接已销毁，其 RemoteObject 随之释放。
                        break;
                    }
                    tracing::warn!(request_id=operation.id(), kind=?error.kind(), stage=error.stage(), session, "CDP cleanup unconfirmed");
                    // 未完成对象释放时不积累远端资源，也不继续提供输入服务。
                    let _ = connection.inner.stop.send(true);
                    break;
                }
            }
        });
    }
}
