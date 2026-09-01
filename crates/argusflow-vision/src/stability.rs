//! 稳定帧门控；把动画、caret 和滚动过程与 OCR 请求隔离。

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::{
    diff::{DiffConfig, compute_dirty_map},
    error::VisionError,
    image::CapturedFrame,
    source::FrameSubscription,
};

/// 稳定帧门控参数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StabilityConfig {
    /// 必须连续观察到的低变化帧数量。
    pub min_frames: u32,
    /// 低于该变化比例才认为当前帧稳定。
    pub stable_changed_ratio: f32,
    /// 等待稳定的最大时长。
    pub timeout: Duration,
}

impl Default for StabilityConfig {
    fn default() -> Self {
        Self {
            min_frames: 2,
            stable_changed_ratio: 0.01,
            timeout: Duration::from_millis(800),
        }
    }
}

impl StabilityConfig {
    /// 检查稳定性参数。
    pub fn validate(self) -> Result<Self, VisionError> {
        if self.min_frames == 0 || !(0.0..=1.0).contains(&self.stable_changed_ratio) {
            return Err(VisionError::Protocol {
                message: "stability frames and ratio are invalid".to_owned(),
            });
        }
        Ok(self)
    }
}

/// 门控状态，供 inspector 和 metrics 解释当前为何没有发起 OCR。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum StabilityState {
    /// 仍在等待连续稳定帧。
    Collecting {
        /// 当前连续低变化帧数。
        consecutive_frames: u32,
        /// 最近一次变化比例。
        changed_ratio: f32,
    },
    /// 已达到稳定条件。
    Stable,
}

/// 将连续帧转换成稳定帧的有状态门控器。
#[derive(Debug)]
pub struct StableFrameGate {
    /// 稳定性参数。
    config: StabilityConfig,
    /// 差分参数。
    diff_config: DiffConfig,
    /// 上一次观察到的帧。
    previous: Option<Arc<CapturedFrame>>,
    /// 当前连续稳定帧计数。
    consecutive_frames: u32,
    /// 最近状态。
    state: StabilityState,
}

impl StableFrameGate {
    /// 创建使用指定参数的门控器。
    pub fn new(config: StabilityConfig, diff_config: DiffConfig) -> Result<Self, VisionError> {
        Ok(Self {
            config: config.validate()?,
            diff_config: diff_config.validate()?,
            previous: None,
            consecutive_frames: 0,
            state: StabilityState::Collecting {
                consecutive_frames: 0,
                changed_ratio: 1.0,
            },
        })
    }

    /// 创建默认门控器。
    pub fn default_gate() -> Self {
        Self::new(StabilityConfig::default(), DiffConfig::default())
            .expect("default stability and diff configuration is valid")
    }

    /// 观察一帧；达到稳定条件时返回该帧。
    pub fn observe(
        &mut self,
        frame: Arc<CapturedFrame>,
    ) -> Result<Option<Arc<CapturedFrame>>, VisionError> {
        let changed_ratio = match self.previous.as_deref() {
            None => {
                // 初始帧没有参照物，但它仍可作为连续稳定窗口的第一帧。
                0.0
            }
            Some(previous)
                if previous.window != frame.window
                    || previous.topology_generation != frame.topology_generation =>
            {
                // topology 变化即使像素暂时相同，也必须重新收集完整稳定窗口。
                1.0
            }
            Some(previous) => {
                compute_dirty_map(Some(previous), &frame, self.diff_config)?.changed_area_ratio
            }
        };
        self.previous = Some(frame.clone());
        if changed_ratio <= self.config.stable_changed_ratio {
            self.consecutive_frames = self.consecutive_frames.saturating_add(1);
        } else {
            self.consecutive_frames = 0;
        }
        if self.consecutive_frames >= self.config.min_frames {
            self.state = StabilityState::Stable;
            Ok(Some(frame))
        } else {
            self.state = StabilityState::Collecting {
                consecutive_frames: self.consecutive_frames,
                changed_ratio,
            };
            Ok(None)
        }
    }

    /// 返回门控当前状态。
    pub const fn state(&self) -> StabilityState {
        self.state
    }

    /// 使用同一窗口上一张已确认稳定的帧作为刷新基线。
    ///
    /// 基线只用于判断后续画面是否变化，不计作本轮新鲜帧。WGC 的有界池可能在 UI
    /// 动作期间积压旧帧；若把基线计为第一张稳定帧，紧接着读到的一张旧尾帧就会错误
    /// 结束刷新，永远不给缓冲释放后的新画面到达机会。
    pub fn seed(&mut self, frame: Arc<CapturedFrame>) {
        self.previous = Some(frame);
        self.consecutive_frames = 0;
        self.state = StabilityState::Collecting {
            consecutive_frames: 0,
            changed_ratio: 0.0,
        };
    }

    /// 清空上一帧和计数，供窗口重建或 topology 变化后重新开始。
    pub fn reset(&mut self) {
        self.previous = None;
        self.consecutive_frames = 0;
        self.state = StabilityState::Collecting {
            consecutive_frames: 0,
            changed_ratio: 1.0,
        };
    }

    /// 在明确 deadline 内从订阅中等待一张稳定帧。
    pub async fn wait_for_stable(
        &mut self,
        subscription: &dyn FrameSubscription,
    ) -> Result<Arc<CapturedFrame>, VisionError> {
        let deadline = Instant::now() + self.config.timeout;
        let mut observed_frames = u32::from(self.previous.is_some());
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(if observed_frames == 0 {
                    VisionError::FrameTimeout {
                        timeout_ms: self.config.timeout.as_millis() as u64,
                    }
                } else {
                    VisionError::FrameUnstable {
                        observed_frames,
                        timeout_ms: self.config.timeout.as_millis() as u64,
                    }
                });
            }
            let frame = match subscription.next(remaining).await {
                Ok(frame) => frame,
                Err(VisionError::FrameTimeout { .. }) if observed_frames > 0 => {
                    // 捕获源在交付最新帧后直到 deadline 都没有新 FrameArrived，等价于该帧
                    // 在完整稳定窗口内保持静止；静态窗口不应被迫制造第二张重复帧。
                    let latest = self.previous.clone().ok_or(VisionError::FrameTimeout {
                        timeout_ms: self.config.timeout.as_millis() as u64,
                    })?;
                    let current_topology = subscription.current_topology_generation().await?;
                    if current_topology != latest.topology_generation {
                        return Err(VisionError::SceneStale);
                    }
                    self.state = StabilityState::Stable;
                    return Ok(latest);
                }
                Err(error) => return Err(error),
            };
            observed_frames = observed_frames.saturating_add(1);
            if let Some(stable) = self.observe(frame)? {
                return Ok(stable);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use argusflow_core::WindowIdentity;

    use super::*;
    use crate::{
        MemoryFrameSource, WindowFrameSource,
        frame::{FrameId, QpcTimestamp, TopologyGeneration},
    };

    fn frame(id: u64, fill: u8) -> Arc<CapturedFrame> {
        Arc::new(
            CapturedFrame::from_bgra8(
                FrameId::new(id),
                TopologyGeneration::new(0),
                WindowIdentity {
                    handle: 2,
                    process_id: 3,
                },
                QpcTimestamp::new(id),
                8,
                8,
                96,
                96,
                32,
                vec![fill; 8 * 8 * 4],
            )
            .expect("fixture frame is valid"),
        )
    }

    #[test]
    fn gate_requires_two_stable_frames() {
        let mut gate = StableFrameGate::default_gate();
        assert!(gate.observe(frame(1, 0)).expect("first frame").is_none());
        assert!(gate.observe(frame(2, 0)).expect("second frame").is_some());
        assert_eq!(gate.state(), StabilityState::Stable);
    }

    #[test]
    fn changed_frame_resets_consecutive_count() {
        let mut gate = StableFrameGate::default_gate();
        assert!(gate.observe(frame(1, 0)).expect("first frame").is_none());
        assert!(
            gate.observe(frame(2, 255))
                .expect("changed frame")
                .is_none()
        );
        assert_eq!(
            gate.state(),
            StabilityState::Collecting {
                consecutive_frames: 0,
                changed_ratio: 1.0,
            }
        );
    }

    /// 已缓存场景只能作为比较基线；第一张积压旧帧不得提前结束强制刷新。
    #[test]
    fn seeded_baseline_requires_fresh_frames_before_refresh_completes() {
        let old = frame(1, 0);
        let updated = frame(3, 255);
        let mut gate = StableFrameGate::default_gate();
        gate.seed(old.clone());

        assert!(
            gate.observe(frame(2, 0))
                .expect("queued old frame")
                .is_none()
        );
        assert!(
            gate.observe(updated)
                .expect("first updated frame")
                .is_none()
        );
        assert!(
            gate.observe(frame(4, 255))
                .expect("first low-change updated frame")
                .is_none()
        );
        assert!(
            gate.observe(frame(5, 255))
                .expect("settled updated frame")
                .is_some()
        );
    }

    #[tokio::test]
    async fn a_quiet_stream_accepts_its_latest_observed_frame() {
        let source = MemoryFrameSource::new();
        let identity = WindowIdentity {
            handle: 2,
            process_id: 3,
        };
        source.insert(identity, vec![frame(7, 0)]);
        let subscription = source
            .open(identity, crate::CapturePolicy::default())
            .await
            .expect("memory subscription should open");
        let mut gate = StableFrameGate::default_gate();

        let stable = gate
            .wait_for_stable(subscription.as_ref())
            .await
            .expect("silence after one frame proves that the latest frame settled");

        assert_eq!(stable.frame_id, FrameId::new(7));
        assert_eq!(gate.state(), StabilityState::Stable);
    }
}
