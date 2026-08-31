//! 动作路由中的视觉后置条件生命周期。

use argusflow_core::{
    ActionOutcome, AutomationError, BackendKind, PreparedAqlQuery, PreparedVisualPostcondition,
    RunTraceContext, TargetWaitPolicy,
};

use crate::{
    VisualBaseline, VisualBaselineRequirement, VisualVerificationProvider,
    VisualVerificationResult, WindowContext,
};

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
    let (query, stable_context, requirement): (
        &PreparedAqlQuery,
        &[PreparedAqlQuery],
        VisualBaselineRequirement,
    ) = match postcondition {
        PreparedVisualPostcondition::MatchAdded {
            query,
            stable_context,
        } => (
            query,
            stable_context.as_slice(),
            VisualBaselineRequirement::AnyCount,
        ),
        PreparedVisualPostcondition::MatchRemoved {
            query,
            stable_context,
        } => (
            query,
            stable_context.as_slice(),
            VisualBaselineRequirement::AtLeastOne,
        ),
        PreparedVisualPostcondition::MatchPresent { query } => {
            (query, &[], VisualBaselineRequirement::AnyCount)
        }
    };
    provider
        .capture_baseline(window, query, stable_context, requirement, trace_context)
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
        PreparedVisualPostcondition::MatchAdded { .. } => {
            provider.verify_match_added(baseline, wait).await
        }
        PreparedVisualPostcondition::MatchRemoved { .. } => {
            provider.verify_match_removed(baseline, wait).await
        }
        PreparedVisualPostcondition::MatchPresent { .. } => {
            provider.verify_match_present(baseline, wait).await
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
        VisualVerificationResult::MatchAddedConfirmed {
            baseline_count,
            current_count,
            added_count,
        } => {
            outcome.outputs.insert("confirmed".to_owned(), true.into());
            outcome.message.push_str(&format!(
                "；视觉确认同一窗口内出现 {added_count} 个新匹配（动作前 {baseline_count}，当前 {current_count}）",
            ));
        }
        VisualVerificationResult::MatchPresentConfirmed => {
            outcome.outputs.insert("confirmed".to_owned(), true.into());
            outcome
                .message
                .push_str("；视觉确认目标已在新鲜画面中唯一匹配");
        }
        VisualVerificationResult::MatchRemovedConfirmed {
            baseline_count,
            current_count,
            removed_count,
        } => {
            outcome.outputs.insert("confirmed".to_owned(), true.into());
            outcome.message.push_str(&format!(
                "；视觉确认同一窗口内 {removed_count} 个旧匹配已消失（动作前 {baseline_count}，当前 {current_count}）",
            ));
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
