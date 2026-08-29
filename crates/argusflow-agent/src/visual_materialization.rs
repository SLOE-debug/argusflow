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
    let is_visual_click = matches!(
        action,
        AutomationAction::Click { target }
            if matches!(&target.locator, TargetLocator::Visual { .. })
    );
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
    if !target.backend_policy.allows(BackendKind::SendInput)
        || !matches!(&target.locator, TargetLocator::Visual { .. })
    {
        return Ok(None);
    }
    let Some(prepared_target) = prepared_target else {
        return Ok(None);
    };
    let PreparedTargetLocator::Visual { query } = prepared_target.locator() else {
        return Ok(None);
    };
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
                    query: query.text.clone(),
                    timeout_ms: wait.timeout_ms,
                    details: String::new(),
                })
            }
            Some(deadline) => {
                tokio::time::timeout_at(deadline, materializer.materialize(window, query, &plan))
                    .await
                    .map_err(|_| AutomationError::TargetWaitTimeout {
                        query: query.text.clone(),
                        timeout_ms: wait.timeout_ms,
                        details: String::new(),
                    })
                    .and_then(|result| result)
            }
            None => materializer.materialize(window, query, &plan).await,
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
