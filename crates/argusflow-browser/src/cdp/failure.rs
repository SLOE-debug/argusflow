//! CDP 传输、生命周期与协议契约的强类型失败分类。

use thiserror::Error;

/// CDP 连接、target/session 生命周期或远端方法错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum CdpProtocolError {
    /// WebSocket 无法建立或在请求期间断开。
    #[error("CDP transport is unavailable: {message}")]
    TransportUnavailable {
        /// 不包含页面内容的传输错误摘要。
        message: String,
    },
    /// 扁平 CDP session 已被 Chromium 分离。
    #[error("CDP session {session_id} was detached: {reason}")]
    SessionDetached {
        /// `Target.attachToTarget` 返回的 session id。
        session_id: String,
        /// 协议事件或方法错误提供的稳定原因摘要。
        reason: String,
    },
    /// renderer 或其它 target 进程已崩溃。
    #[error(
        "CDP target crashed (target={target_id:?}, status={status:?}, error_code={error_code:?})"
    )]
    TargetCrashed {
        /// target id；仅 Inspector session 事件可能暂时无法提供。
        target_id: Option<String>,
        /// Chromium 提供的终止状态。
        status: Option<String>,
        /// Chromium 提供的进程终止码。
        error_code: Option<i64>,
    },
    /// page target 已被销毁或关闭。
    #[error("CDP target was closed (target={target_id:?})")]
    TargetClosed {
        /// 已关闭 target id；只有方法错误先于注册时可能缺失。
        target_id: Option<String>,
    },
    /// Chromium 拒绝了一个仍处于可用生命周期内的方法调用。
    #[error("CDP method {method} was rejected ({code}): {message}")]
    MethodRejected {
        /// 失败的方法名称。
        method: String,
        /// CDP 错误代码。
        code: i64,
        /// CDP 错误消息。
        message: String,
    },
    /// Chromium 响应缺少当前协议调用要求的字段。
    #[error("invalid CDP response: {message}")]
    InvalidResponse {
        /// 响应结构错误说明。
        message: String,
    },
}

impl CdpProtocolError {
    /// 判断错误是否表示当前连接、session 或 target 已无法继续执行。
    pub(crate) const fn is_backend_unavailable(&self) -> bool {
        matches!(
            self,
            Self::TransportUnavailable { .. }
                | Self::SessionDetached { .. }
                | Self::TargetCrashed { .. }
                | Self::TargetClosed { .. }
        )
    }

    /// 把 CDP 方法错误收敛为生命周期终止或普通方法拒绝。
    pub(crate) fn classify_method_rejection(
        method: String,
        code: i64,
        message: String,
        session_id: Option<&str>,
        target_id: Option<&str>,
    ) -> Self {
        let normalized = message.trim().to_ascii_lowercase();
        if normalized.contains(SESSION_NOT_FOUND_MESSAGE)
            && let Some(session_id) = session_id
        {
            return Self::SessionDetached {
                session_id: session_id.to_owned(),
                reason: message,
            };
        }
        if TARGET_CLOSED_MESSAGES
            .iter()
            .any(|known| normalized.contains(known))
        {
            return Self::TargetClosed {
                target_id: target_id.map(str::to_owned),
            };
        }
        Self::MethodRejected {
            method,
            code,
            message,
        }
    }
}

/// Chromium 在 flat session 不存在时返回的稳定消息片段。
const SESSION_NOT_FOUND_MESSAGE: &str = "session with given id not found";

/// 只包含明确表示 target 生命周期结束的 Chromium 消息片段。
const TARGET_CLOSED_MESSAGES: &[&str] = &[
    "target closed",
    "no target with given id found",
    "not attached to an active page",
];

#[cfg(test)]
mod tests {
    use super::CdpProtocolError;

    #[test]
    fn missing_flat_session_is_classified_as_unavailable() {
        let error = CdpProtocolError::classify_method_rejection(
            "Runtime.evaluate".to_owned(),
            -32_001,
            "Session with given id not found.".to_owned(),
            Some("session-7"),
            Some("target-2"),
        );

        assert!(matches!(
            error,
            CdpProtocolError::SessionDetached { session_id, .. }
                if session_id == "session-7"
        ));
    }

    #[test]
    fn closed_target_is_classified_as_unavailable() {
        let error = CdpProtocolError::classify_method_rejection(
            "Runtime.evaluate".to_owned(),
            -32_000,
            "Target closed.".to_owned(),
            Some("session-7"),
            Some("target-2"),
        );

        assert!(matches!(
            error,
            CdpProtocolError::TargetClosed { target_id: Some(target_id) }
                if target_id == "target-2"
        ));
    }

    #[test]
    fn execution_context_error_remains_method_rejection() {
        let error = CdpProtocolError::classify_method_rejection(
            "Runtime.evaluate".to_owned(),
            -32_000,
            "Execution context was destroyed.".to_owned(),
            Some("session-7"),
            Some("target-2"),
        );

        assert!(matches!(error, CdpProtocolError::MethodRejected { .. }));
    }
}
