//! 物理输入前后的视觉 Scene 差分验证。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use argusflow_agent::{
    VisualBaseline, VisualVerificationProvider, VisualVerificationResult, WindowContext,
};
use argusflow_core::{
    AutomationError, BackendKind, TargetWaitMode, TargetWaitPolicy, VisualQuery, WindowIdentity,
};
use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    AppScene, PhysicalRect, SceneRefreshPolicy, VisionError, VisionRuntime, WindowInventory,
    matching_app_nodes,
};

/// 与一次非幂等输入绑定的视觉文字空间基线。
#[derive(Debug)]
struct VisualBaselineSnapshot {
    /// 捕获基线时冻结的主窗口，防止 token 被跨应用消费。
    window: WindowContext,
    /// 查询在动作前已经存在的窗口内文字位置。
    facts: Vec<VisualTextFact>,
}

/// OCR 文字在某个窗口二维坐标系中的实例身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisualTextFact {
    /// 文字所属 HWND/PID，隔离进程内不同 capture surface。
    window: WindowIdentity,
    /// 帧本地物理 bbox；相交区域视为同一个抗抖动文字实例。
    bounds: PhysicalRect,
}

/// 使用共享 VisionRuntime 完成发送前基线和发送后新增事实验证。
#[derive(Debug)]
pub struct VisionPostconditionVerifier {
    /// 与读取和点击共用的捕获、OCR 与 Scene cache。
    runtime: Arc<VisionRuntime>,
    /// 枚举 AppSession 进程全部可见窗口。
    inventory: Arc<dyn WindowInventory>,
    /// opaque token 到短期基线的唯一所有权表。
    baselines: Mutex<HashMap<Uuid, VisualBaselineSnapshot>>,
}

impl VisionPostconditionVerifier {
    /// 创建绑定共享视觉世界与窗口注册表的后置条件验证器。
    pub fn new(runtime: Arc<VisionRuntime>, inventory: Arc<dyn WindowInventory>) -> Self {
        Self {
            runtime,
            inventory,
            baselines: Mutex::new(HashMap::new()),
        }
    }

    /// 强制从当前稳定帧重建完整窗口 Scene，避免动作前后复用同一份 cache。
    async fn observe(
        &self,
        process_id: u32,
        query: &VisualQuery,
    ) -> Result<Vec<VisualTextFact>, AutomationError> {
        let mut policy = SceneRefreshPolicy::small();
        policy.force_refresh = true;
        policy.force_full_ocr = true;
        policy.max_age = Duration::ZERO;
        let scene = self
            .runtime
            .current_app_scene(self.inventory.as_ref(), process_id, &policy, None)
            .await
            .map_err(map_vision_error)?;
        Ok(matching_facts(&scene, query))
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
    ) -> Result<VisualBaseline, AutomationError> {
        let facts = self.observe(window.process_id, query).await?;
        let baseline = VisualBaseline::new(window.clone());
        self.baselines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                baseline.token(),
                VisualBaselineSnapshot {
                    window: window.clone(),
                    facts,
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
        let started_at = tokio::time::Instant::now();
        let deadline = match wait.mode {
            TargetWaitMode::None => None,
            TargetWaitMode::Bounded => Some(started_at + Duration::from_millis(wait.timeout_ms)),
        };
        loop {
            let observation = match deadline {
                Some(deadline) => {
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        return Ok(VisualVerificationResult::Uncertain {
                            reason: format!(
                                "在 {}ms 内未取得可用于确认发送结果的新视觉 Scene",
                                wait.timeout_ms
                            ),
                        });
                    }
                    tokio::time::timeout(remaining, self.observe(snapshot.window.process_id, query))
                        .await
                        .map_err(|_| AutomationError::OutcomeUnknown {
                            backend: BackendKind::SendInput,
                            message: format!(
                                "发送后视觉观察在 {}ms 截止时间内未完成",
                                wait.timeout_ms
                            ),
                        })??
                }
                None => self.observe(snapshot.window.process_id, query).await?,
            };
            if has_new_fact(&snapshot.facts, &observation) {
                return Ok(VisualVerificationResult::Confirmed);
            }
            let Some(deadline) = deadline else {
                return Ok(VisualVerificationResult::Rejected {
                    reason: "发送后完整视觉 Scene 中没有出现新的目标文字事实".to_owned(),
                });
            };
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return Ok(VisualVerificationResult::Rejected {
                    reason: format!("发送后 {}ms 内没有出现新的目标文字事实", wait.timeout_ms),
                });
            }
            let poll_interval = Duration::from_millis(wait.poll_interval_ms);
            tokio::time::sleep(poll_interval.min(deadline.saturating_duration_since(now))).await;
        }
    }
}

/// 将查询命中的 OCR 节点投影为不包含文字内容的二维事实。
fn matching_facts(scene: &AppScene, query: &VisualQuery) -> Vec<VisualTextFact> {
    matching_app_nodes(scene, query)
        .into_iter()
        .map(|candidate| VisualTextFact {
            window: candidate.window.identity,
            bounds: candidate.node.bbox,
        })
        .collect()
}

/// 只有当前文字位置无法与任一旧位置对应时，才视为动作产生的新视觉事实。
fn has_new_fact(baseline: &[VisualTextFact], current: &[VisualTextFact]) -> bool {
    current.iter().any(|candidate| {
        !baseline.iter().any(|existing| {
            candidate.window == existing.window && candidate.bounds.intersects(existing.bounds)
        })
    })
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
    fn overlapping_bbox_is_the_same_antijitter_text_fact() {
        let window = WindowIdentity {
            handle: 68_116,
            process_id: 12_468,
        };
        let baseline = [fact(window, 700, 650, 120, 30)];
        let current = [fact(window, 701, 649, 120, 31)];

        assert!(!has_new_fact(&baseline, &current));
    }

    #[test]
    fn same_text_at_a_new_conversation_bbox_is_a_new_fact() {
        let window = WindowIdentity {
            handle: 68_116,
            process_id: 12_468,
        };
        let input_fact = fact(window, 700, 650, 120, 30);
        let sent_bubble = fact(window, 760, 520, 120, 30);

        assert!(has_new_fact(&[input_fact], &[sent_bubble]));
    }

    #[test]
    fn identical_bbox_in_another_window_is_not_the_same_fact() {
        let main = WindowIdentity {
            handle: 68_116,
            process_id: 12_468,
        };
        let popup = WindowIdentity {
            handle: 721_788,
            process_id: 12_468,
        };

        assert!(has_new_fact(
            &[fact(main, 10, 20, 100, 30)],
            &[fact(popup, 10, 20, 100, 30)],
        ));
    }

    /// 创建满足不变量的二维文字事实。
    fn fact(window: WindowIdentity, x: i32, y: i32, width: u32, height: u32) -> VisualTextFact {
        VisualTextFact {
            window,
            bounds: PhysicalRect::new(x, y, width, height).expect("fixture bounds should be valid"),
        }
    }
}
