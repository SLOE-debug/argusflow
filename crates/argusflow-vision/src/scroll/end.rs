//! 组合式滚动到底检测。

use serde::{Deserialize, Serialize};

use super::model::PageSnapshot;

/// 到底检测使用的阈值。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScrollEndConfig {
    /// 连续多少次低位移才允许判定到底。
    pub consecutive_low_shift: u8,
    /// 低于该像素位移视为没有内容移动。
    pub min_shift_px: f32,
}

impl ScrollEndConfig {
    /// 检查到底检测参数。
    pub fn validate(self) -> Result<Self, crate::error::VisionError> {
        if self.consecutive_low_shift == 0
            || !(self.min_shift_px.is_finite() && self.min_shift_px > 0.0)
        {
            return Err(crate::error::VisionError::Protocol {
                message: "scroll end configuration is invalid".to_owned(),
            });
        }
        Ok(self)
    }
}

impl Default for ScrollEndConfig {
    fn default() -> Self {
        Self {
            consecutive_low_shift: 2,
            min_shift_px: 2.0,
        }
    }
}

/// 滚动到底检测器；不因单个重复文字直接结束。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScrollEndDetector {
    /// 检测配置。
    pub config: ScrollEndConfig,
    /// 连续低位移计数。
    pub low_shift_count: u8,
}

impl Default for ScrollEndDetector {
    fn default() -> Self {
        Self::new(ScrollEndConfig::default())
    }
}

impl ScrollEndDetector {
    /// 创建到底检测器。
    pub const fn new(config: ScrollEndConfig) -> Self {
        Self {
            config,
            low_shift_count: 0,
        }
    }

    /// 组合位移、内容签名和底部锚点不变证据判断是否到底。
    pub fn observe(
        &mut self,
        previous: &PageSnapshot,
        current: &PageSnapshot,
        actual_shift_px: f32,
    ) -> bool {
        let low_shift = actual_shift_px.is_finite() && actual_shift_px < self.config.min_shift_px;
        let same_content = previous.content_signature == current.content_signature;
        let same_bottom_anchors = previous.anchors == current.anchors;
        if low_shift && same_content && same_bottom_anchors {
            self.low_shift_count = self.low_shift_count.saturating_add(1);
        } else {
            self.low_shift_count = 0;
        }
        self.low_shift_count >= self.config.consecutive_low_shift
    }
}
