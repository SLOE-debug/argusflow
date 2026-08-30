use std::{sync::Arc, time::Duration};

use argusflow_core::{
    AutomationAction, AutomationError, BackendKind, PreparedAutomationTarget,
    PreparedTargetLocator, TargetLocator, TargetWaitMode, TargetWaitPolicy,
};

use crate::{
    ExecutionContext, MaterializedTarget, PreparedTargetMaterialization,
    PreparedTargetMaterializer, VisualMaterializationPlan,
};

/// Router 每次动作只创建一次的视觉查询准备结果。
pub(crate) struct PreparedVisualMaterialization {
    /// 已编译 AQL 的平台执行对象。
    execution: Arc<dyn PreparedTargetMaterialization>,
    /// 按策略冻结的 OCR 物化阶段。
    plan: VisualMaterializationPlan,
    /// 等待错误使用的稳定 AQL 摘要。
    query_summary: String,
}

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

/// 只为显式 OCR + SendInput AQL Click 执行一次由 Planner 冻结的视觉物化链。
pub(crate) fn prepare(
    materializer: Option<&dyn PreparedTargetMaterializer>,
    action: &AutomationAction,
    prepared_target: Option<&PreparedAutomationTarget>,
) -> Result<Option<PreparedVisualMaterialization>, AutomationError> {
    let AutomationAction::Click { target } = action else {
        return Ok(None);
    };
    if !visual_input_locator(target) {
        return Ok(None);
    }
    let Some(prepared_target) = prepared_target else {
        return Ok(None);
    };
    let Some(materializer) = materializer else {
        return Err(AutomationError::BackendUnavailable {
            backend: BackendKind::OcrSmall,
            message: "visual click has no Planner target materializer".to_owned(),
        });
    };
    let plan = VisualMaterializationPlan::from_policy(
        prepared_target.backend_policy(),
        &materializer.available_stages(),
    )
    .ok_or_else(|| AutomationError::BackendUnavailable {
        backend: BackendKind::OcrSmall,
        message: "backend policy and runtime availability leave no visual materialization stage"
            .to_owned(),
    })?;
    let execution = materializer.prepare(prepared_target.locator())?;
    Ok(Some(PreparedVisualMaterialization {
        execution,
        plan,
        query_summary: locator_summary(prepared_target.locator()),
    }))
}

/// 使用同一个冻结查询对象轮询最新 Scene，避免重复解析或编译正则。
pub(crate) async fn materialize(
    prepared: Option<&PreparedVisualMaterialization>,
    context: &ExecutionContext,
    wait: TargetWaitPolicy,
    deadline: Option<tokio::time::Instant>,
    trace_context: Option<&argusflow_core::RunTraceContext>,
) -> Result<Option<MaterializedTarget>, AutomationError> {
    let Some(prepared) = prepared else {
        return Ok(None);
    };
    let Some(window) = context.foreground_window.as_ref() else {
        return Ok(None);
    };
    loop {
        let result = match deadline {
            Some(deadline) if tokio::time::Instant::now() >= deadline => {
                Err(AutomationError::TargetWaitTimeout {
                    query: prepared.query_summary.clone(),
                    timeout_ms: wait.timeout_ms,
                    details: String::new(),
                })
            }
            Some(deadline) => tokio::time::timeout_at(
                deadline,
                prepared
                    .execution
                    .materialize(window, &prepared.plan, trace_context),
            )
            .await
            .map_err(|_| AutomationError::TargetWaitTimeout {
                query: prepared.query_summary.clone(),
                timeout_ms: wait.timeout_ms,
                details: String::new(),
            })
            .and_then(|result| result),
            None => {
                prepared
                    .execution
                    .materialize(window, &prepared.plan, trace_context)
                    .await
            }
        };
        match result {
            Ok(target) => return Ok(Some(target)),
            Err(
                error @ (AutomationError::TargetNotFound { .. }
                | AutomationError::ObservationIncomplete { .. }),
            ) if wait.mode == TargetWaitMode::Bounded
                && deadline.is_some_and(|deadline| tokio::time::Instant::now() < deadline) =>
            {
                let poll_interval = Duration::from_millis(wait.poll_interval_ms.max(1));
                let Some(deadline) = deadline else {
                    return Err(error);
                };
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                tokio::time::sleep(poll_interval.min(remaining)).await;
            }
            Err(
                error @ (AutomationError::TargetNotFound { .. }
                | AutomationError::ObservationIncomplete { .. }),
            ) if wait.mode == TargetWaitMode::Bounded => {
                let (query, details) = target_error_parts(error);
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

/// 消费可等待查询错误并返回统一的查询与诊断字段。
fn target_error_parts(error: AutomationError) -> (String, String) {
    match error {
        AutomationError::TargetNotFound { query, details }
        | AutomationError::ObservationIncomplete { query, details } => (query, details),
        _ => ("OCR query".to_owned(), String::new()),
    }
}

/// 判断目标是否显式要求由 Vision 物化后交给 SendInput。
fn visual_input_locator(target: &argusflow_core::AutomationTarget) -> bool {
    if !target.backend_policy.allows(BackendKind::SendInput) {
        return false;
    }
    match &target.locator {
        TargetLocator::Query { .. } => {
            target.backend_policy.allow.len() == 2
                && target.backend_policy.allow.contains(&BackendKind::OcrSmall)
                && target
                    .backend_policy
                    .allow
                    .contains(&BackendKind::SendInput)
                && target.backend_policy.deny.is_empty()
        }
        TargetLocator::Coordinate { .. } | TargetLocator::Focused => false,
    }
}

/// 返回等待错误使用的稳定查询摘要。
fn locator_summary(locator: &PreparedTargetLocator) -> String {
    match locator {
        PreparedTargetLocator::Query { source, .. } => source.clone(),
        PreparedTargetLocator::Coordinate { point } => format!("{},{}", point.x, point.y),
        PreparedTargetLocator::Focused => "focused".to_owned(),
    }
}
