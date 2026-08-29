use std::{any::Any, future::Future, panic::AssertUnwindSafe};

use argusflow_core::{ExecutionEventKind, ExecutionEventPayload};
use futures_util::FutureExt;

use super::run_context::NodeOutcome;

/// PreparedNode 交给 Engine 发出的单个节点内事件。
#[derive(Debug)]
pub struct NodeEvent {
    /// 节点内事件类别。
    pub kind: ExecutionEventKind,
    /// 可选说明；不得包含未被节点语义明确允许记录的敏感数据。
    pub message: Option<String>,
    /// 可安全传给前端的结构化载荷。
    pub payload: Option<ExecutionEventPayload>,
}

/// 一个节点完成后的结构化结果与可观察事件。
#[derive(Debug, Default)]
pub struct NodeExecution {
    /// 保存到 RunContext 的值和资源端口。
    pub outcome: NodeOutcome,
    /// 在 NodeSucceeded 前按顺序发出的节点内事件。
    pub events: Vec<NodeEvent>,
}

/// 等待节点 future，同时把 unwind panic 转换为可进入执行事件流的错误摘要。
pub(crate) async fn catch_node_unwind<T>(future: impl Future<Output = T>) -> Result<T, String> {
    AssertUnwindSafe(future)
        .catch_unwind()
        .await
        .map_err(panic_message)
}

/// 提取常见 panic payload；未知载荷不泄漏内部类型或地址。
fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return (*message).to_owned();
    }
    "节点执行发生了未分类 panic".to_owned()
}

#[cfg(test)]
mod tests {
    use super::catch_node_unwind;

    #[tokio::test]
    async fn panic_is_converted_to_an_error_message() {
        let result = catch_node_unwind(async { panic!("probe panic") }).await;

        assert_eq!(result.expect_err("panic must not escape"), "probe panic");
    }
}
