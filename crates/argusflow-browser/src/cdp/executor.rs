//! 已冻结 CSS 查询上的单次页面内批量动作执行。

use std::{collections::BTreeMap, time::Duration};

use argusflow_core::{ActionOutcome, AutomationAction, AutomationError, BackendKind};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{CdpPageSession, CdpPlanExpr, page_script::build_page_action_script};

/// 执行 CSS fast path，并在短暂页面渲染期间按有界策略重试未命中查询。
pub(crate) async fn execute_cdp_action(
    session: &CdpPageSession,
    action: &AutomationAction,
    expression: &CdpPlanExpr,
    query: &str,
) -> Result<ActionOutcome, AutomationError> {
    let script = build_page_action_script(expression, action)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let result = evaluate_action(session, &script).await?;
        match result.status.as_str() {
            "not_found" if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            "not_found" => {
                return Err(AutomationError::TargetNotFound {
                    query: query.to_owned(),
                });
            }
            "ambiguous" => {
                return Err(AutomationError::AmbiguousTarget {
                    query: query.to_owned(),
                    matches: result.matches,
                });
            }
            "ok" => {
                return Ok(ActionOutcome {
                    backend: BackendKind::BrowserCdp,
                    message: result.message,
                    outputs: result.outputs,
                    diagnostic_evidence: Vec::new(),
                });
            }
            status => {
                return Err(AutomationError::BackendFailed {
                    backend: BackendKind::BrowserCdp,
                    message: format!("unknown page action status '{status}'"),
                });
            }
        }
    }
}

/// 单次 `Runtime.evaluate` 往返完成查询、动作适配和输出投影。
async fn evaluate_action(
    session: &CdpPageSession,
    script: &str,
) -> Result<PageActionResult, AutomationError> {
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
        .await
        .map_err(|error| AutomationError::BackendFailed {
            backend: BackendKind::BrowserCdp,
            message: error.to_string(),
        })?;
    if let Some(exception) = response.get("exceptionDetails") {
        return Err(AutomationError::BackendFailed {
            backend: BackendKind::BrowserCdp,
            message: exception
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("page evaluation failed")
                .to_owned(),
        });
    }
    let value = response.pointer("/result/value").cloned().ok_or_else(|| {
        AutomationError::BackendFailed {
            backend: BackendKind::BrowserCdp,
            message: "Runtime.evaluate did not return a by-value result".to_owned(),
        }
    })?;
    serde_json::from_value::<PageActionResult>(value).map_err(|error| {
        AutomationError::BackendFailed {
            backend: BackendKind::BrowserCdp,
            message: format!("invalid page action result: {error}"),
        }
    })
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
