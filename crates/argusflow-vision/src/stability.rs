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

mod noise;

pub use noise::{TemporalNoiseConfig, TemporalNoiseMask};

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
    /// 过滤短期周期性闪烁的时序 mask。
    noise_mask: TemporalNoiseMask,
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
            noise_mask: TemporalNoiseMask::default(),
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
                self.noise_mask.clear();
                1.0
            }
            Some(previous) => {
                let dirty = compute_dirty_map(Some(previous), &frame, self.diff_config)?;
                self.noise_mask
                    .observe(previous, &frame, &dirty)?
                    .changed_area_ratio
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

    /// 清空上一帧和计数，供窗口重建或 topology 变化后重新开始。
    pub fn reset(&mut self) {
        self.previous = None;
        self.consecutive_frames = 0;
        self.noise_mask.clear();
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
        let mut observed_frames = 0_u32;
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
            let frame = subscription.next(remaining).await?;
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
    use crate::frame::{FrameId, QpcTimestamp, TopologyGeneration};

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
}
