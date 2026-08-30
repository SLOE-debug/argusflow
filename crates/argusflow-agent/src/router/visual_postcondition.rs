//! 动作路由中的视觉后置条件生命周期。

use argusflow_core::{
    ActionOutcome, AutomationError, BackendKind, PreparedVisualPostcondition, RunTraceContext,
    TargetWaitPolicy, VisualQuery,
};

use crate::{VisualBaseline, VisualVerificationProvider, VisualVerificationResult, WindowContext};

/// 在动作提交前冻结窗口内的目标数量与稳定上下文。
pub(super) async fn capture_baseline(
    provider: Option<&dyn VisualVerificationProvider>,
    postcondition: Option<&PreparedVisualPostcondition>,
    window: Option<&WindowContext>,
    trace_context: Option<RunTraceContext>,
) -> Result<Option<VisualBaseline>, AutomationError> {
    let Some(postcondition) = postcondition else {
        return Ok(None);
    };
    let provider = provider.ok_or_else(|| AutomationError::BackendUnavailable {
        backend: BackendKind::SendInput,
        message: "visual postcondition provider is not configured".to_owned(),
    })?;
    let window = window.ok_or_else(|| AutomationError::BackendUnavailable {
        backend: BackendKind::SendInput,
        message: "visual postcondition requires a frozen window context".to_owned(),
    })?;
    let (query, stable_context): (&VisualQuery, &[VisualQuery]) = match postcondition {
        PreparedVisualPostcondition::NewText {
            query,
            stable_context,
        } => (query, stable_context.as_slice()),
        PreparedVisualPostcondition::TextPresent { query } => (query, &[]),
    };
    provider
        .capture_baseline(window, query, stable_context, trace_context)
        .await
        .map(Some)
}

/// 在动作未能完成时释放尚未消费的视觉基线。
pub(super) async fn discard_baseline(
    provider: Option<&dyn VisualVerificationProvider>,
    baseline: &mut Option<VisualBaseline>,
) {
    if let (Some(provider), Some(baseline)) = (provider, baseline.take()) {
        provider.discard_baseline(baseline).await;
    }
}

/// 消费动作前基线，并把严格视觉确认写入动作结果。
pub(super) async fn verify(
    provider: Option<&dyn VisualVerificationProvider>,
    postcondition: Option<&PreparedVisualPostcondition>,
    baseline: &mut Option<VisualBaseline>,
    wait: TargetWaitPolicy,
    outcome: &mut ActionOutcome,
) -> Result<(), AutomationError> {
    let Some(postcondition) = postcondition else {
        return Ok(());
    };
    let provider = provider.ok_or_else(|| AutomationError::OutcomeUnknown {
        backend: outcome.backend,
        message: "视觉后置条件 provider 在动作后不可用".to_owned(),
    })?;
    let baseline = baseline
        .take()
        .ok_or_else(|| AutomationError::OutcomeUnknown {
            backend: outcome.backend,
            message: "视觉后置条件缺少动作前基线".to_owned(),
        })?;
    let verification = match postcondition {
        PreparedVisualPostcondition::NewText { query, .. } => {
            provider.verify_new_text(baseline, query, wait).await
        }
        PreparedVisualPostcondition::TextPresent { query } => {
            provider.verify_text_present(baseline, query, wait).await
        }
    }
    .map_err(|error| match error {
        AutomationError::OutcomeUnknown { .. } => error,
        other => AutomationError::OutcomeUnknown {
            backend: BackendKind::SendInput,
            message: format!("visual postcondition verification failed: {other}"),
        },
    })?;
    match verification {
        VisualVerificationResult::NewTextConfirmed {
            baseline_count,
            current_count,
        } => {
            outcome.outputs.insert("confirmed".to_owned(), true.into());
            outcome.message.push_str(&format!(
                "；视觉确认同一窗口内目标文字数量由 {baseline_count} 增至 {current_count}",
            ));
        }
        VisualVerificationResult::TextPresentConfirmed => {
            outcome.outputs.insert("confirmed".to_owned(), true.into());
            outcome
                .message
                .push_str("；视觉确认目标文字已在新鲜画面中唯一出现");
        }
        VisualVerificationResult::Rejected { reason }
        | VisualVerificationResult::Uncertain { reason } => {
            return Err(AutomationError::OutcomeUnknown {
                backend: outcome.backend,
                message: reason,
            });
        }
    }
    Ok(())
}
