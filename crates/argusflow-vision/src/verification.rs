//! 物理输入前后的单窗口 AQL 视觉事实验证。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use argusflow_agent::{
    VisualBaseline, VisualBaselineRequirement, VisualVerificationProvider,
    VisualVerificationResult, WindowContext,
};
use argusflow_core::{
    AutomationError, BackendKind, PreparedAqlQuery, RunTraceContext, TargetWaitMode,
    TargetWaitPolicy, WindowIdentity,
};
use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    FrameId, SceneRefreshPolicy, TopologyGeneration, VisionError, VisionQueryPlan, VisionRuntime,
    VisualNode, VisualScene, compile_vision_query, query::evaluate_window_query,
};

mod match_delta;
mod stable_context;

use match_delta::{added_match_count, removed_match_count};
use stable_context::{StableContextSnapshot, capture_stable_context, stable_context_preserved};

/// 与一次非幂等输入绑定的主窗口视觉基线。
#[derive(Debug)]
struct VisualBaselineSnapshot {
    /// 捕获基线时冻结的主窗口，防止 token 被跨应用消费。
    window: WindowContext,
    /// 完整 OCR 基线所属的单调帧。
    frame_id: FrameId,
    /// 捕获时的窗口拓扑代数；动作期间 resize 会使连续性失效。
    topology_generation: TopologyGeneration,
    /// 已编译且绑定动态参数的目标查询。
    target_query: FrozenVisionQuery,
    /// 动作前目标查询的全部空间实例。
    target_matches: Vec<VisualNode>,
    /// 动作前必须唯一存在且在动作后保持原位的上下文。
    stable_context: Vec<StableContextSnapshot>,
    /// 保存动作前后 OCR 帧所需的运行轨迹身份。
    trace_context: Option<RunTraceContext>,
}

/// 一条只供当前动作重复求值的 Vision 查询计划。
#[derive(Debug)]
struct FrozenVisionQuery {
    /// 已完成能力检查和正则预编译的计划。
    plan: VisionQueryPlan,
    /// 用于错误和追踪的原始 AQL 源码。
    source: String,
}

impl FrozenVisionQuery {
    /// 将 Runtime 冻结的 AQL 编译为 Vision 可执行计划。
    fn compile(query: &PreparedAqlQuery) -> Result<Self, AutomationError> {
        let plan = compile_vision_query(query.query()).map_err(|error| {
            AutomationError::BackendFailed {
                backend: BackendKind::OcrSmall,
                message: format!("视觉后置条件 AQL 不受 Vision 支持：{error}"),
            }
        })?;
        Ok(Self {
            plan,
            source: query.source().to_owned(),
        })
    }

    /// 在完整单窗口 Scene 上执行冻结计划并返回可持有的节点快照。
    fn evaluate(&self, scene: &Arc<VisualScene>) -> Result<Vec<VisualNode>, AutomationError> {
        let result = evaluate_window_query(scene, &self.plan, &self.source)?;
        let _metrics = result.metrics;
        Ok(result.matches)
    }
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

    /// 在剩余预算内取得一份强制完整 OCR 的窗口 Scene；正常耗尽预算时返回 `None`。
    async fn observe_with_deadline(
        &self,
        window: &WindowContext,
        trace_context: Option<&RunTraceContext>,
        deadline: Option<tokio::time::Instant>,
    ) -> Result<Option<Arc<VisualScene>>, AutomationError> {
        match deadline {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    return Ok(None);
                }
                match tokio::time::timeout(remaining, self.observe(window, trace_context)).await {
                    Ok(scene) => scene.map(Some),
                    Err(_) => Ok(None),
                }
            }
            None => self.observe(window, trace_context).await.map(Some),
        }
    }
}

#[async_trait]
impl VisualVerificationProvider for VisionPostconditionVerifier {
    async fn capture_baseline(
        &self,
        window: &WindowContext,
        query: &PreparedAqlQuery,
        stable_context: &[PreparedAqlQuery],
        requirement: VisualBaselineRequirement,
        trace_context: Option<RunTraceContext>,
    ) -> Result<VisualBaseline, AutomationError> {
        let target_query = FrozenVisionQuery::compile(query)?;
        let stable_queries = stable_context
            .iter()
            .map(FrozenVisionQuery::compile)
            .collect::<Result<Vec<_>, _>>()?;
        let scene = self.observe(window, trace_context.as_ref()).await?;
        let target_matches = target_query.evaluate(&scene)?;
        if requirement == VisualBaselineRequirement::AtLeastOne && target_matches.is_empty() {
            return Err(AutomationError::TargetNotFound {
                query: target_query.source.clone(),
                details: " was not present before the action".to_owned(),
            });
        }
        let context_snapshots = capture_stable_context(&scene, stable_queries)?;
        let baseline = VisualBaseline::new(window.clone());
        self.baselines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                baseline.token(),
                VisualBaselineSnapshot {
                    window: window.clone(),
                    frame_id: scene.frame_id,
                    topology_generation: scene.topology_generation,
                    target_query,
                    target_matches,
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

    async fn verify_match_added(
        &self,
        baseline: VisualBaseline,
        wait: TargetWaitPolicy,
    ) -> Result<VisualVerificationResult, AutomationError> {
        let snapshot = self.take_baseline(&baseline)?;
        let deadline = verification_deadline(wait);
        let mut observed_fresh_scene = false;
        let mut last_reason = "尚未取得动作后的新鲜完整画面".to_owned();
        loop {
            let Some(scene) = self
                .observe_with_deadline(&snapshot.window, snapshot.trace_context.as_ref(), deadline)
                .await?
            else {
                break;
            };
            if scene.topology_generation != snapshot.topology_generation {
                return Ok(VisualVerificationResult::Uncertain {
                    reason: "动作期间窗口大小或显示拓扑发生变化，无法比较前后视觉实例".to_owned(),
                });
            }
            if scene.frame_id > snapshot.frame_id {
                observed_fresh_scene = true;
                match stable_context_preserved(&snapshot.stable_context, &scene) {
                    Ok(()) => {
                        let current_matches = snapshot.target_query.evaluate(&scene)?;
                        let added_count =
                            added_match_count(&snapshot.target_matches, &current_matches);
                        if added_count > 0 {
                            return Ok(VisualVerificationResult::MatchAddedConfirmed {
                                baseline_count: snapshot.target_matches.len(),
                                current_count: current_matches.len(),
                                added_count,
                            });
                        }
                        last_reason = format!(
                            "当前 {} 个匹配都与动作前同文本实例相交，没有发现新实例",
                            current_matches.len(),
                        );
                    }
                    Err(reason) => last_reason = reason,
                }
            }
            if !wait_again(deadline, wait).await {
                break;
            }
        }
        Ok(match_added_deadline_result(
            observed_fresh_scene,
            &last_reason,
            wait,
        ))
    }

    async fn verify_match_removed(
        &self,
        baseline: VisualBaseline,
        wait: TargetWaitPolicy,
    ) -> Result<VisualVerificationResult, AutomationError> {
        let snapshot = self.take_baseline(&baseline)?;
        let deadline = verification_deadline(wait);
        let mut observed_fresh_scene = false;
        let mut last_reason = "尚未取得动作后的新鲜完整画面".to_owned();
        loop {
            let Some(scene) = self
                .observe_with_deadline(&snapshot.window, snapshot.trace_context.as_ref(), deadline)
                .await?
            else {
                break;
            };
            if scene.topology_generation != snapshot.topology_generation {
                return Ok(VisualVerificationResult::Uncertain {
                    reason: "动作期间窗口大小或显示拓扑发生变化，无法比较前后视觉实例".to_owned(),
                });
            }
            if scene.frame_id > snapshot.frame_id {
                observed_fresh_scene = true;
                match stable_context_preserved(&snapshot.stable_context, &scene) {
                    Ok(()) => {
                        let current_matches = snapshot.target_query.evaluate(&scene)?;
                        let removed_count =
                            removed_match_count(&snapshot.target_matches, &current_matches);
                        if removed_count > 0 {
                            return Ok(VisualVerificationResult::MatchRemovedConfirmed {
                                baseline_count: snapshot.target_matches.len(),
                                current_count: current_matches.len(),
                                removed_count,
                            });
                        }
                        last_reason = format!(
                            "动作前 {} 个匹配在当前 {} 个匹配中仍有同文本相交实例，没有发现旧实例消失",
                            snapshot.target_matches.len(),
                            current_matches.len(),
                        );
                    }
                    Err(reason) => last_reason = reason,
                }
            }
            if !wait_again(deadline, wait).await {
                break;
            }
        }
        Ok(match_removed_deadline_result(
            observed_fresh_scene,
            &last_reason,
            wait,
        ))
    }

    async fn verify_match_present(
        &self,
        baseline: VisualBaseline,
        wait: TargetWaitPolicy,
    ) -> Result<VisualVerificationResult, AutomationError> {
        let snapshot = self.take_baseline(&baseline)?;
        let deadline = verification_deadline(wait);
        let mut last_count = None;
        loop {
            let Some(scene) = self
                .observe_with_deadline(&snapshot.window, snapshot.trace_context.as_ref(), deadline)
                .await?
            else {
                break;
            };
            if scene.topology_generation != snapshot.topology_generation {
                return Ok(VisualVerificationResult::Uncertain {
                    reason: "动作期间窗口大小或显示拓扑发生变化，无法确认视觉连续性".to_owned(),
                });
            }
            if scene.frame_id > snapshot.frame_id {
                let current_count = snapshot.target_query.evaluate(&scene)?.len();
                last_count = Some(current_count);
                if current_count == 1 {
                    return Ok(VisualVerificationResult::MatchPresentConfirmed);
                }
            }
            if !wait_again(deadline, wait).await {
                break;
            }
        }
        Ok(match_present_deadline_result(last_count, wait))
    }
}

/// 将新增匹配轮询的正常截止转换为基于既有视觉证据的三态结果。
fn match_added_deadline_result(
    observed_fresh_scene: bool,
    last_reason: &str,
    wait: TargetWaitPolicy,
) -> VisualVerificationResult {
    if observed_fresh_scene {
        VisualVerificationResult::Rejected {
            reason: format!(
                "动作后 {}ms 内未确认新增匹配：{last_reason}",
                wait.timeout_ms,
            ),
        }
    } else {
        VisualVerificationResult::Uncertain {
            reason: format!(
                "在 {}ms 内未取得晚于动作前基线的完整视觉 Scene",
                wait.timeout_ms,
            ),
        }
    }
}

/// 将匹配消失轮询的正常截止转换为基于既有视觉证据的三态结果。
fn match_removed_deadline_result(
    observed_fresh_scene: bool,
    last_reason: &str,
    wait: TargetWaitPolicy,
) -> VisualVerificationResult {
    if observed_fresh_scene {
        VisualVerificationResult::Rejected {
            reason: format!(
                "动作后 {}ms 内未确认匹配消失：{last_reason}",
                wait.timeout_ms,
            ),
        }
    } else {
        VisualVerificationResult::Uncertain {
            reason: format!(
                "在 {}ms 内未取得晚于动作前基线的完整视觉 Scene",
                wait.timeout_ms,
            ),
        }
    }
}

/// 将存在性轮询的正常截止转换为最后一帧的明确拒绝或无观测不确定结果。
fn match_present_deadline_result(
    last_count: Option<usize>,
    wait: TargetWaitPolicy,
) -> VisualVerificationResult {
    match last_count {
        Some(current_count) => VisualVerificationResult::Rejected {
            reason: format!(
                "动作后 {}ms 内目标未唯一匹配（最后完整画面命中 {current_count} 项）",
                wait.timeout_ms,
            ),
        },
        None => VisualVerificationResult::Uncertain {
            reason: format!(
                "在 {}ms 内未取得晚于动作前基线的完整视觉 Scene",
                wait.timeout_ms,
            ),
        },
    }
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
#[path = "verification/tests.rs"]
mod tests;
