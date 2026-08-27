//! 已冻结 CSS 查询上的单次页面内批量动作执行。

use std::collections::BTreeMap;

use argusflow_core::{ActionOutcome, AutomationAction, AutomationError, BackendKind};
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;

use super::{
    CdpPageSession, CdpPlanExpr, failure::CdpProtocolError, page_script::build_page_action_script,
};

/// 执行一次 CSS fast path；目标未出现时立即交回 PreparedPlan 决定是否等待。
pub(crate) async fn execute_cdp_action(
    session: &CdpPageSession,
    action: &AutomationAction,
    expression: &CdpPlanExpr,
    query: &str,
) -> Result<ActionOutcome, AutomationError> {
    let script = build_page_action_script(expression, action)?;
    let result = evaluate_action(session, &script)
        .await
        .map_err(CdpExecutionError::into_automation_error)?;
    match result.status.as_str() {
        "not_found" => Err(AutomationError::TargetNotFound {
            query: query.to_owned(),
        }),
        "ambiguous" => Err(AutomationError::AmbiguousTarget {
            query: query.to_owned(),
            matches: result.matches,
        }),
        "ok" => Ok(ActionOutcome {
            backend: BackendKind::BrowserCdp,
            message: result.message,
            outputs: result.outputs,
            diagnostic_evidence: Vec::new(),
        }),
        status => Err(CdpExecutionError::InvalidExecutorResponse {
            message: format!("unknown page action status '{status}'"),
        }
        .into_automation_error()),
    }
}

/// 单次 `Runtime.evaluate` 往返完成查询、动作适配和输出投影。
async fn evaluate_action(
    session: &CdpPageSession,
    script: &str,
) -> Result<PageActionResult, CdpExecutionError> {
    let response = session
        .command(
            "Runtime.evaluate",
            json!({
                "expression": script,
                "awaitPromise": true,
                "returnByValue": true,
                "userGesture": true,
            }),
        )
        .await?;
    if let Some(exception) = response.get("exceptionDetails") {
        return Err(CdpExecutionError::PageScriptFailure {
            message: exception
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("page evaluation failed")
                .to_owned(),
        });
    }
    let value = response.pointer("/result/value").cloned().ok_or_else(|| {
        CdpExecutionError::InvalidExecutorResponse {
            message: "Runtime.evaluate did not return a by-value result".to_owned(),
        }
    })?;
    serde_json::from_value::<PageActionResult>(value).map_err(|error| {
        CdpExecutionError::InvalidExecutorResponse {
            message: format!("invalid page action result: {error}"),
        }
    })
}

/// CDP 动作执行层的完整失败分类。
#[derive(Debug, Error)]
enum CdpExecutionError {
    /// WebSocket、session、target、method 或协议响应错误。
    #[error(transparent)]
    Protocol(#[from] CdpProtocolError),
    /// Runtime.evaluate 成功返回，但页面解释器执行抛出异常。
    #[error("CDP page script failed: {message}")]
    PageScriptFailure {
        /// 不包含页面业务数据的异常摘要。
        message: String,
    },
    /// 页面解释器返回值不符合冻结 DTO 契约。
    #[error("invalid CDP executor response: {message}")]
    InvalidExecutorResponse {
        /// 缺失字段、未知状态或反序列化错误。
        message: String,
    },
}

impl CdpExecutionError {
    /// 按生命周期可用性映射到 PreparedPlan 的回退契约。
    fn into_automation_error(self) -> AutomationError {
        match self {
            Self::Protocol(error) if error.is_backend_unavailable() => {
                AutomationError::BackendUnavailable {
                    backend: BackendKind::BrowserCdp,
                    message: error.to_string(),
                }
            }
            error => AutomationError::BackendFailed {
                backend: BackendKind::BrowserCdp,
                message: error.to_string(),
            },
        }
    }
}

/// 页面内函数返回的最小稳定 DTO。
#[derive(Debug, Deserialize)]
struct PageActionResult {
    /// `ok`、`not_found` 或 `ambiguous`。
    status: String,
    /// 唯一性错误使用的语义候选数量。
    #[serde(default)]
    matches: usize,
    /// 不包含业务读取正文的执行说明。
    #[serde(default)]
    message: String,
    /// 动作公开的结构化输出。
    #[serde(default)]
    outputs: BTreeMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use argusflow_core::{AutomationError, BackendKind};

    use super::{CdpExecutionError, CdpProtocolError};

    #[test]
    fn transport_failure_allows_backend_fallback() {
        let error = CdpExecutionError::Protocol(CdpProtocolError::TransportUnavailable {
            message: "connection closed".to_owned(),
        })
        .into_automation_error();

        assert!(matches!(
            error,
            AutomationError::BackendUnavailable {
                backend: BackendKind::BrowserCdp,
                message,
            } if message == "CDP transport is unavailable: connection closed"
        ));
    }

    #[test]
    fn detached_session_allows_backend_fallback() {
        let error = CdpExecutionError::Protocol(CdpProtocolError::SessionDetached {
            session_id: "session-7".to_owned(),
            reason: "target closed".to_owned(),
        })
        .into_automation_error();

        assert!(matches!(error, AutomationError::BackendUnavailable { .. }));
    }

    #[test]
    fn crashed_target_allows_backend_fallback() {
        let error = CdpExecutionError::Protocol(CdpProtocolError::TargetCrashed {
            target_id: Some("target-2".to_owned()),
            status: Some("crashed".to_owned()),
            error_code: Some(5),
        })
        .into_automation_error();

        assert!(matches!(error, AutomationError::BackendUnavailable { .. }));
    }

    #[test]
    fn rejected_method_does_not_trigger_backend_fallback() {
        let error = CdpExecutionError::Protocol(CdpProtocolError::MethodRejected {
            method: "Runtime.evaluate".to_owned(),
            code: -32_000,
            message: "Execution context was destroyed.".to_owned(),
        })
        .into_automation_error();

        assert!(matches!(error, AutomationError::BackendFailed { .. }));
    }

    #[test]
    fn invalid_protocol_response_remains_backend_failure() {
        let error = CdpExecutionError::Protocol(CdpProtocolError::InvalidResponse {
            message: "missing result".to_owned(),
        })
        .into_automation_error();

        assert!(matches!(
            error,
            AutomationError::BackendFailed {
                backend: BackendKind::BrowserCdp,
                ..
            }
        ));
    }

    #[test]
    fn page_script_failure_does_not_trigger_backend_fallback() {
        let error = CdpExecutionError::PageScriptFailure {
            message: "selector interpreter failed".to_owned(),
        }
        .into_automation_error();

        assert!(matches!(error, AutomationError::BackendFailed { .. }));
    }
}
