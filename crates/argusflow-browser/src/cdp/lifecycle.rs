//! CDP connection、flat session 与 target 的共享健康状态。

use std::{
    collections::HashMap,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use serde_json::Value;

use super::failure::CdpProtocolError;

/// actor 与轻量 command handle 共享的只读快路径状态。
#[derive(Debug, Default)]
pub(super) struct CdpConnectionHealth {
    /// 同步锁只保护小型字符串映射，永不跨 await 持有。
    state: RwLock<CdpConnectionState>,
}

impl CdpConnectionHealth {
    /// 记录 flat session 与 target 的稳定绑定关系。
    pub(super) fn register_session(&self, session_id: String, target_id: String) {
        self.write_state()
            .session_targets
            .insert(session_id, target_id);
    }

    /// 返回当前连接或指定 session 已知的终止错误。
    pub(super) fn unavailable_error(&self, session_id: Option<&str>) -> Option<CdpProtocolError> {
        let state = self.read_state();
        if let Some(error) = &state.transport_failure {
            return Some(error.clone());
        }
        let session_id = session_id?;
        if let Some(error) = state.session_failures.get(session_id) {
            return Some(error.clone());
        }
        let target_id = state.session_targets.get(session_id)?;
        state.target_failures.get(target_id).cloned()
    }

    /// 返回 session 当前绑定的 target id，供方法错误分类使用。
    pub(super) fn target_id(&self, session_id: Option<&str>) -> Option<String> {
        let session_id = session_id?;
        self.read_state().session_targets.get(session_id).cloned()
    }

    /// 记录 WebSocket 终止并返回可发送给所有 pending 请求的错误。
    pub(super) fn mark_transport_unavailable(&self, message: String) -> CdpProtocolError {
        let error = CdpProtocolError::TransportUnavailable { message };
        self.write_state().transport_failure = Some(error.clone());
        error
    }

    /// 解析并记录一个无 request id 的生命周期事件。
    pub(super) fn observe_event(&self, message: &Value) -> Option<CdpProtocolError> {
        let method = message.get("method").and_then(Value::as_str)?;
        let params = message.get("params").unwrap_or(message);
        let event = match method {
            "Target.detachedFromTarget" => CdpLifecycleEvent::SessionDetached {
                session_id: text_field(params, "sessionId")?,
                reason: "Target.detachedFromTarget".to_owned(),
            },
            "Target.targetCrashed" => CdpLifecycleEvent::TargetCrashed {
                target_id: Some(text_field(params, "targetId")?),
                status: optional_text_field(params, "status"),
                error_code: params.get("errorCode").and_then(Value::as_i64),
            },
            "Target.targetDestroyed" => CdpLifecycleEvent::TargetClosed {
                target_id: text_field(params, "targetId")?,
            },
            "Inspector.detached" => CdpLifecycleEvent::SessionDetached {
                session_id: text_field(message, "sessionId")?,
                reason: optional_text_field(params, "reason")
                    .unwrap_or_else(|| "Inspector.detached".to_owned()),
            },
            "Inspector.targetCrashed" => CdpLifecycleEvent::SessionTargetCrashed {
                session_id: text_field(message, "sessionId")?,
            },
            _ => return None,
        };
        Some(self.record_event(event))
    }

    /// 记录由方法错误识别出的生命周期终止，供后续请求预检。
    pub(super) fn record_unavailable(&self, error: CdpProtocolError) {
        if !error.is_backend_unavailable() {
            return;
        }
        let mut state = self.write_state();
        match &error {
            CdpProtocolError::TransportUnavailable { .. } => {
                state.transport_failure = Some(error.clone());
            }
            CdpProtocolError::SessionDetached { session_id, .. } => {
                state
                    .session_failures
                    .insert(session_id.clone(), error.clone());
            }
            CdpProtocolError::TargetCrashed {
                target_id: Some(target_id),
                ..
            }
            | CdpProtocolError::TargetClosed {
                target_id: Some(target_id),
            } => {
                state
                    .target_failures
                    .insert(target_id.clone(), error.clone());
            }
            CdpProtocolError::TargetCrashed {
                target_id: None, ..
            }
            | CdpProtocolError::TargetClosed { target_id: None }
            | CdpProtocolError::MethodRejected { .. }
            | CdpProtocolError::InvalidResponse { .. } => {}
        }
    }

    /// 将结构化生命周期事件写入 session/target 状态表。
    fn record_event(&self, event: CdpLifecycleEvent) -> CdpProtocolError {
        let error = match event {
            CdpLifecycleEvent::SessionDetached { session_id, reason } => {
                CdpProtocolError::SessionDetached { session_id, reason }
            }
            CdpLifecycleEvent::TargetCrashed {
                target_id,
                status,
                error_code,
            } => CdpProtocolError::TargetCrashed {
                target_id,
                status,
                error_code,
            },
            CdpLifecycleEvent::TargetClosed { target_id } => CdpProtocolError::TargetClosed {
                target_id: Some(target_id),
            },
            CdpLifecycleEvent::SessionTargetCrashed { session_id } => {
                let target_id = self.target_id(Some(&session_id));
                let error = CdpProtocolError::TargetCrashed {
                    target_id,
                    status: None,
                    error_code: None,
                };
                self.write_state()
                    .session_failures
                    .insert(session_id, error.clone());
                return error;
            }
        };
        self.record_unavailable(error.clone());
        error
    }

    /// 忽略 poison 并保留 actor 的错误广播能力。
    fn read_state(&self) -> RwLockReadGuard<'_, CdpConnectionState> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 忽略 poison 并保留 actor 的生命周期更新能力。
    fn write_state(&self) -> RwLockWriteGuard<'_, CdpConnectionState> {
        self.state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// 连接内所有 session/target 的终止状态。
#[derive(Debug, Default)]
struct CdpConnectionState {
    /// 整条 WebSocket 的终止错误。
    transport_failure: Option<CdpProtocolError>,
    /// flat session 到 target 的稳定映射。
    session_targets: HashMap<String, String>,
    /// 仅影响单个 flat session 的终止错误。
    session_failures: HashMap<String, CdpProtocolError>,
    /// 影响同一 target 所有 session 的终止错误。
    target_failures: HashMap<String, CdpProtocolError>,
}

/// 当前执行器关心的封闭生命周期事件。
enum CdpLifecycleEvent {
    /// flat session 被分离。
    SessionDetached { session_id: String, reason: String },
    /// root Target domain 提供完整 target crash 信息。
    TargetCrashed {
        target_id: Option<String>,
        status: Option<String>,
        error_code: Option<i64>,
    },
    /// root Target domain 报告 target 被销毁。
    TargetClosed { target_id: String },
    /// page Inspector domain 报告当前 session target 崩溃。
    SessionTargetCrashed { session_id: String },
}

/// 读取必需的非空字符串字段。
fn text_field(value: &Value, field: &str) -> Option<String> {
    optional_text_field(value, field).filter(|value| !value.is_empty())
}

/// 读取可选字符串字段。
fn optional_text_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::CdpConnectionHealth;
    use crate::cdp::failure::CdpProtocolError;

    #[test]
    fn detached_event_marks_only_its_session_unavailable() {
        let health = CdpConnectionHealth::default();
        health.register_session("session-a".to_owned(), "target-a".to_owned());
        health.register_session("session-b".to_owned(), "target-b".to_owned());

        health.observe_event(&json!({
            "method": "Target.detachedFromTarget",
            "params": { "sessionId": "session-a", "targetId": "target-a" }
        }));

        assert!(matches!(
            health.unavailable_error(Some("session-a")),
            Some(CdpProtocolError::SessionDetached { .. })
        ));
        assert_eq!(health.unavailable_error(Some("session-b")), None);
    }

    #[test]
    fn target_crash_marks_every_registered_session_unavailable() {
        let health = CdpConnectionHealth::default();
        health.register_session("session-a".to_owned(), "target-a".to_owned());
        health.register_session("session-b".to_owned(), "target-a".to_owned());

        health.observe_event(&json!({
            "method": "Target.targetCrashed",
            "params": { "targetId": "target-a", "status": "crashed", "errorCode": 5 }
        }));

        assert!(matches!(
            health.unavailable_error(Some("session-a")),
            Some(CdpProtocolError::TargetCrashed { .. })
        ));
        assert!(health.unavailable_error(Some("session-b")).is_some());
    }
}
