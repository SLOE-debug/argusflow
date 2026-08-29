use std::time::Duration;

use argusflow_core::{
    AutomationAction, AutomationError, BackendKind, PreparedAutomationTarget,
    PreparedTargetLocator, TargetLocator, TargetWaitMode, TargetWaitPolicy,
};

use crate::{
    ExecutionContext, MaterializedTarget, PreparedTargetMaterializer, VisualMaterializationPlan,
};

/// 返回 Visual Click 目标物化和输入前重试共享的单一截止时间。
pub(crate) fn deadline(
    action: &AutomationAction,
    wait: TargetWaitPolicy,
) -> Option<tokio::time::Instant> {
    let is_visual_click =
        matches!(action, AutomationAction::Click { target } if visual_input_locator(target));
    (is_visual_click && wait.mode == TargetWaitMode::Bounded)
        .then(|| tokio::time::Instant::now() + Duration::from_millis(wait.timeout_ms))
}

/// 只为 Visual Click 执行一次由 Planner 冻结的视觉物化链。
pub(crate) async fn materialize(
    materializer: Option<&dyn PreparedTargetMaterializer>,
    action: &AutomationAction,
    context: &ExecutionContext,
    prepared_target: Option<&PreparedAutomationTarget>,
    wait: TargetWaitPolicy,
    deadline: Option<tokio::time::Instant>,
) -> Result<Option<MaterializedTarget>, AutomationError> {
    let AutomationAction::Click { target } = action else {
        return Ok(None);
    };
    if !visual_input_locator(target) {
        return Ok(None);
    }
    let Some(prepared_target) = prepared_target else {
        return Ok(None);
    };
    let query_summary = locator_summary(prepared_target.locator());
    let Some(window) = context.foreground_window.as_ref() else {
        return Ok(None);
    };
    let Some(materializer) = materializer else {
        return Err(AutomationError::BackendUnavailable {
            backend: BackendKind::VisualCache,
            message: "visual click has no Planner target materializer".to_owned(),
        });
    };
    let plan = VisualMaterializationPlan::from_policy(
        prepared_target.backend_policy(),
        &materializer.available_stages(),
    )
    .ok_or_else(|| AutomationError::BackendUnavailable {
        backend: BackendKind::VisualCache,
        message: "backend policy and runtime availability leave no visual materialization stage"
            .to_owned(),
    })?;
    loop {
        let result = match deadline {
            Some(deadline) if tokio::time::Instant::now() >= deadline => {
                Err(AutomationError::TargetWaitTimeout {
                    query: query_summary.clone(),
                    timeout_ms: wait.timeout_ms,
                    details: String::new(),
                })
            }
            Some(deadline) => tokio::time::timeout_at(
                deadline,
                materializer.materialize(window, prepared_target.locator(), &plan),
            )
            .await
            .map_err(|_| AutomationError::TargetWaitTimeout {
                query: query_summary.clone(),
                timeout_ms: wait.timeout_ms,
                details: String::new(),
            })
            .and_then(|result| result),
            None => {
                materializer
                    .materialize(window, prepared_target.locator(), &plan)
                    .await
            }
        };
        match result {
            Ok(target) => return Ok(Some(target)),
            Err(AutomationError::TargetNotFound { query, details })
                if wait.mode == TargetWaitMode::Bounded
                    && deadline.is_some_and(|deadline| tokio::time::Instant::now() < deadline) =>
            {
                let poll_interval = Duration::from_millis(wait.poll_interval_ms.max(1));
                let Some(deadline) = deadline else {
                    return Err(AutomationError::TargetNotFound { query, details });
                };
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                tokio::time::sleep(poll_interval.min(remaining)).await;
            }
            Err(AutomationError::TargetNotFound { query, details })
                if wait.mode == TargetWaitMode::Bounded =>
            {
                return Err(AutomationError::TargetWaitTimeout {
                    query,
                    timeout_ms: wait.timeout_ms,
                    details,
                });
            }
            Err(error) => return Err(error),
        }
    }
}

/// 判断目标是否显式要求由 Vision 物化后交给 SendInput。
fn visual_input_locator(target: &argusflow_core::AutomationTarget) -> bool {
    if !target.backend_policy.allows(BackendKind::SendInput) {
        return false;
    }
    match &target.locator {
        TargetLocator::Visual { .. } => true,
        TargetLocator::Query { query } => {
            query.language_version == argusflow_core::QueryLanguageVersion::V2
                && target
                    .backend_policy
                    .allow
                    .contains(&BackendKind::SendInput)
                && target.backend_policy.allow.iter().any(|backend| {
                    matches!(
                        backend,
                        BackendKind::VisualCache
                            | BackendKind::OcrTiny
                            | BackendKind::OcrSmall
                            | BackendKind::OcrMedium
                            | BackendKind::GuiGrounding
                    )
                })
        }
        TargetLocator::Coordinate { .. } | TargetLocator::Focused => false,
    }
}

/// 返回等待错误使用的稳定查询摘要。
fn locator_summary(locator: &PreparedTargetLocator) -> String {
    match locator {
        PreparedTargetLocator::Visual { query } => query.text.clone(),
        PreparedTargetLocator::Query { query, .. } => query.source.clone(),
        PreparedTargetLocator::Coordinate { point } => format!("{},{}", point.x, point.y),
        PreparedTargetLocator::Focused => "focused".to_owned(),
    }
}
