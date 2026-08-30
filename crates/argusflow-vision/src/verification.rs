//! 物理输入前后的单窗口视觉事实验证。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use argusflow_agent::{
    VisualBaseline, VisualVerificationProvider, VisualVerificationResult, WindowContext,
};
use argusflow_core::{
    AutomationError, BackendKind, RunTraceContext, TargetWaitMode, TargetWaitPolicy, VisualQuery,
    WindowIdentity,
};
use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    FrameId, PhysicalRect, SceneRefreshPolicy, VisionError, VisionRuntime, VisualScene,
    matching_nodes,
};

/// 与一次非幂等输入绑定的主窗口视觉基线。
#[derive(Debug)]
struct VisualBaselineSnapshot {
    /// 捕获基线时冻结的主窗口，防止 token 被跨应用消费。
    window: WindowContext,
    /// 完整 OCR 基线所属的单调帧。
    frame_id: FrameId,
    /// 动作前查询区域中的目标文字实例数量。
    target_count: usize,
    /// 动作前必须唯一存在且在动作后保持原位的上下文。
    stable_context: Vec<StableContextSnapshot>,
    /// 保存动作前后 OCR 帧所需的运行轨迹身份。
    trace_context: Option<RunTraceContext>,
}

/// 一条上下文查询及其动作前唯一命中的位置。
#[derive(Debug)]
struct StableContextSnapshot {
    /// 动态值已冻结的文字查询。
    query: VisualQuery,
    /// 动作前唯一文字实例的帧内位置。
    bounds: PhysicalRect,
}

/// 使用共享 VisionRuntime 完成动作前基线和动作后事实验证。
#[derive(Debug)]
pub struct VisionPostconditionVerifier {
    /// 与读取和点击共用的捕获、OCR 与 Scene cache。
    runtime: Arc<VisionRuntime>,
    /// opaque token 到短期基线的唯一所有权表。
    baselines: Mutex<HashMap<Uuid, VisualBaselineSnapshot>>,
}

impl VisionPostconditionVerifier {
    /// 创建绑定共享视觉世界的后置条件验证器。
    pub fn new(runtime: Arc<VisionRuntime>) -> Self {
        Self {
            runtime,
            baselines: Mutex::new(HashMap::new()),
        }
    }

    /// 强制重建冻结主窗口的完整 Scene，排除同进程弹窗和其它窗口的文字。
    async fn observe(
        &self,
        window: &WindowContext,
        trace_context: Option<&RunTraceContext>,
    ) -> Result<Arc<VisualScene>, AutomationError> {
        let mut policy = SceneRefreshPolicy::small();
        policy.force_refresh = true;
        policy.force_full_ocr = true;
        policy.max_age = Duration::ZERO;
        let identity = WindowIdentity {
            handle: window.handle,
            process_id: window.process_id,
        };
        let scene = match trace_context {
            Some(context) => {
                self.runtime
                    .current_scene_for_run(identity, &policy, context)
                    .await
            }
            None => self.runtime.current_scene(identity, &policy).await,
        };
        scene.map_err(map_vision_error)
    }

    /// 原子移除并返回基线，确保一个 token 无法重复确认多次输入。
    fn take_baseline(
        &self,
        baseline: &VisualBaseline,
    ) -> Result<VisualBaselineSnapshot, AutomationError> {
        let snapshot = self
            .baselines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&baseline.token())
            .ok_or_else(|| AutomationError::BackendFailed {
                backend: BackendKind::OcrSmall,
                message: "visual postcondition baseline is missing or already consumed".to_owned(),
            })?;
        if &snapshot.window != baseline.window() {
            return Err(AutomationError::BackendFailed {
                backend: BackendKind::OcrSmall,
                message: "visual postcondition baseline crossed its frozen window boundary"
                    .to_owned(),
            });
        }
        Ok(snapshot)
    }
}

#[async_trait]
impl VisualVerificationProvider for VisionPostconditionVerifier {
    async fn capture_baseline(
        &self,
        window: &WindowContext,
        query: &VisualQuery,
        stable_context: &[VisualQuery],
        trace_context: Option<RunTraceContext>,
    ) -> Result<VisualBaseline, AutomationError> {
        let scene = self.observe(window, trace_context.as_ref()).await?;
        let context_snapshots = capture_stable_context(&scene, stable_context)?;
        let baseline = VisualBaseline::new(window.clone());
        self.baselines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                baseline.token(),
                VisualBaselineSnapshot {
                    window: window.clone(),
                    frame_id: scene.frame_id,
                    target_count: matching_nodes(&scene, query).len(),
                    stable_context: context_snapshots,
                    trace_context,
                },
            );
        Ok(baseline)
    }

    async fn discard_baseline(&self, baseline: VisualBaseline) {
        self.baselines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&baseline.token());
    }

    async fn verify_new_text(
        &self,
        baseline: VisualBaseline,
        query: &VisualQuery,
        wait: TargetWaitPolicy,
    ) -> Result<VisualVerificationResult, AutomationError> {
        let snapshot = self.take_baseline(&baseline)?;
        let deadline = verification_deadline(wait);
        let mut observed_fresh_scene = false;
        let mut last_reason = "尚未取得动作后的新鲜完整画面".to_owned();
        loop {
            let scene = self
                .observe_with_deadline(
                    &snapshot.window,
                    snapshot.trace_context.as_ref(),
                    deadline,
                    wait,
                )
                .await?;
            if scene.frame_id > snapshot.frame_id {
                observed_fresh_scene = true;
                match stable_context_preserved(&snapshot.stable_context, &scene) {
                    Ok(()) => {
                        let current_count = matching_nodes(&scene, query).len();
                        if target_count_increased(snapshot.target_count, current_count) {
                            return Ok(VisualVerificationResult::NewTextConfirmed {
                                baseline_count: snapshot.target_count,
                                current_count,
                            });
                        }
                        last_reason = format!(
                            "同一窗口内目标文字数量未增加（动作前 {}，当前 {current_count}）",
                            snapshot.target_count,
                        );
                    }
                    Err(reason) => last_reason = reason,
                }
            }
            if !wait_again(deadline, wait).await {
                return Ok(if observed_fresh_scene {
                    VisualVerificationResult::Rejected {
                        reason: format!(
                            "发送后 {}ms 内未确认新增文字：{last_reason}",
                            wait.timeout_ms
                        ),
                    }
                } else {
                    VisualVerificationResult::Uncertain {
                        reason: format!(
                            "在 {}ms 内未取得晚于动作前基线的完整视觉 Scene",
                            wait.timeout_ms,
                        ),
                    }
                });
            }
        }
    }

    async fn verify_text_present(
        &self,
        baseline: VisualBaseline,
        query: &VisualQuery,
        wait: TargetWaitPolicy,
    ) -> Result<VisualVerificationResult, AutomationError> {
        let snapshot = self.take_baseline(&baseline)?;
        let deadline = verification_deadline(wait);
        let mut last_count = None;
        loop {
            let scene = self
                .observe_with_deadline(
                    &snapshot.window,
                    snapshot.trace_context.as_ref(),
                    deadline,
                    wait,
                )
                .await?;
            if scene.frame_id > snapshot.frame_id {
                let current_count = matching_nodes(&scene, query).len();
                last_count = Some(current_count);
                if current_count == 1 {
                    return Ok(VisualVerificationResult::TextPresentConfirmed);
                }
            }
            if !wait_again(deadline, wait).await {
                return Ok(match last_count {
                    Some(current_count) => VisualVerificationResult::Rejected {
                        reason: format!(
                            "动作后 {}ms 内目标文字未唯一出现（最后完整画面命中 {current_count} 项）",
                            wait.timeout_ms,
                        ),
                    },
                    None => VisualVerificationResult::Uncertain {
                        reason: format!(
                            "在 {}ms 内未取得晚于动作前基线的完整视觉 Scene",
                            wait.timeout_ms,
                        ),
                    },
                });
            }
        }
    }
}

impl VisionPostconditionVerifier {
    /// 在剩余预算内取得一份强制完整 OCR 的窗口 Scene。
    async fn observe_with_deadline(
        &self,
        window: &WindowContext,
        trace_context: Option<&RunTraceContext>,
        deadline: Option<tokio::time::Instant>,
        wait: TargetWaitPolicy,
    ) -> Result<Arc<VisualScene>, AutomationError> {
        match deadline {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    return Err(AutomationError::OutcomeUnknown {
                        backend: BackendKind::SendInput,
                        message: format!("视觉观察已耗尽 {}ms 截止时间", wait.timeout_ms),
                    });
                }
                tokio::time::timeout(remaining, self.observe(window, trace_context))
                    .await
                    .map_err(|_| AutomationError::OutcomeUnknown {
                        backend: BackendKind::SendInput,
                        message: format!(
                            "发送后视觉观察在 {}ms 截止时间内未完成",
                            wait.timeout_ms,
                        ),
                    })?
            }
            None => self.observe(window, trace_context).await,
        }
    }
}

/// 在动作前要求每条上下文查询都严格唯一，避免向错误会话提交输入。
fn capture_stable_context(
    scene: &VisualScene,
    queries: &[VisualQuery],
) -> Result<Vec<StableContextSnapshot>, AutomationError> {
    queries
        .iter()
        .enumerate()
        .map(|(index, query)| {
            let matches = matching_nodes(scene, query);
            match matches.as_slice() {
                [candidate] => Ok(StableContextSnapshot {
                    query: query.clone(),
                    bounds: candidate.bbox,
                }),
                [] => Err(AutomationError::TargetNotFound {
                    query: format!("visual stable context #{}", index + 1),
                    details: " was not present in the frozen action window".to_owned(),
                }),
                candidates => Err(AutomationError::AmbiguousTarget {
                    query: format!("visual stable context #{}", index + 1),
                    matches: candidates.len(),
                    details: " in the frozen action window".to_owned(),
                }),
            }
        })
        .collect()
}

/// 确保动作后的上下文仍严格唯一，并与动作前事实位置相交。
fn stable_context_preserved(
    expected: &[StableContextSnapshot],
    scene: &VisualScene,
) -> Result<(), String> {
    for (index, snapshot) in expected.iter().enumerate() {
        let matches = matching_nodes(scene, &snapshot.query);
        let [candidate] = matches.as_slice() else {
            return Err(format!(
                "动作后上下文 #{} 不再严格唯一（命中 {} 项）",
                index + 1,
                matches.len(),
            ));
        };
        if !candidate.bbox.intersects(snapshot.bounds) {
            return Err(format!("动作后上下文 #{} 已离开动作前位置", index + 1));
        }
    }
    Ok(())
}

/// 只有同一查询区域中的实例数量严格增加，才允许确认新增文字。
const fn target_count_increased(baseline_count: usize, current_count: usize) -> bool {
    current_count > baseline_count
}

/// 根据等待策略构造独立于动作目标等待的截止时间。
fn verification_deadline(wait: TargetWaitPolicy) -> Option<tokio::time::Instant> {
    match wait.mode {
        TargetWaitMode::None => None,
        TargetWaitMode::Bounded => {
            Some(tokio::time::Instant::now() + Duration::from_millis(wait.timeout_ms))
        }
    }
}

/// 在有界策略下等待下一轮；None 模式只观察一次。
async fn wait_again(deadline: Option<tokio::time::Instant>, wait: TargetWaitPolicy) -> bool {
    let Some(deadline) = deadline else {
        return false;
    };
    let now = tokio::time::Instant::now();
    if now >= deadline {
        return false;
    }
    let poll_interval = Duration::from_millis(wait.poll_interval_ms.max(1));
    tokio::time::sleep(poll_interval.min(deadline.saturating_duration_since(now))).await;
    tokio::time::Instant::now() < deadline
}

/// 将内部捕获/OCR失败映射为后置条件可解释错误。
fn map_vision_error(error: VisionError) -> AutomationError {
    match error {
        VisionError::CaptureUnavailable { message } => AutomationError::ObservationIncomplete {
            query: "visual postcondition".to_owned(),
            details: format!("; {message}"),
        },
        VisionError::WorkerUnavailable { message }
        | VisionError::OcrFailed { message }
        | VisionError::Protocol { message } => AutomationError::BackendUnavailable {
            backend: BackendKind::OcrSmall,
            message,
        },
        other => AutomationError::BackendFailed {
            backend: BackendKind::OcrSmall,
            message: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifted_historical_text_does_not_confirm_a_new_instance() {
        let historical_count_before_action = 1;
        let same_historical_text_after_layout_shift = 1;

        assert!(!target_count_increased(
            historical_count_before_action,
            same_historical_text_after_layout_shift,
        ));
    }

    #[test]
    fn repeated_text_confirms_only_when_instance_count_increases() {
        let historical_count_before_action = 1;
        let historical_and_new_message_count = 2;

        assert!(target_count_increased(
            historical_count_before_action,
            historical_and_new_message_count,
        ));
    }
}
