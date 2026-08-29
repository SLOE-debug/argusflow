//! 滚轮批次、自适应校准、连续性验收和 overshoot recovery 编排。

use serde::{Deserialize, Serialize};

use argusflow_core::WindowIdentity;

use super::{
    end::{ScrollEndConfig, ScrollEndDetector},
    model::{PageSnapshot, ScrollDirection, ScrollRegion, WheelSteps},
    session::{AcceptedPage, PageTransition, ScrollSession},
};
use crate::error::VisionError;

/// 滚动控制器的安全边界。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScrollControllerConfig {
    /// 单次会话允许的反向恢复次数。
    pub max_recovery_attempts: u8,
    /// 到底检测配置。
    pub end: ScrollEndConfig,
}

impl Default for ScrollControllerConfig {
    fn default() -> Self {
        Self {
            max_recovery_attempts: 3,
            end: ScrollEndConfig::default(),
        }
    }
}

impl ScrollControllerConfig {
    /// 检查控制器配置。
    pub fn validate(self) -> Result<Self, VisionError> {
        if self.max_recovery_attempts == 0 {
            return Err(VisionError::Protocol {
                message: "scroll recovery attempts must be non-zero".to_owned(),
            });
        }
        self.end.validate()?;
        Ok(self)
    }
}

/// 一次页面观察后的下一步控制决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollControllerOutcome {
    /// 继续向目标方向发送下一批滚轮输入。
    Continue(WheelSteps),
    /// overshoot 后先反向小步恢复。
    Recover(WheelSteps),
    /// 当前页已经通过连续性验收。
    Accepted(AcceptedPage),
    /// 多个独立信号确认已经滚动到底部。
    EndOfScroll,
}

/// 单窗口、单区域的滚动闭环控制器。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrollController {
    /// 连续性、历史和 wheel 校准状态。
    pub session: ScrollSession,
    /// 组合式到底检测器。
    pub end_detector: ScrollEndDetector,
    /// 控制边界。
    pub config: ScrollControllerConfig,
    /// 当前小批次已经累计的内容位移。
    pub accumulated_shift_px: f32,
}

impl ScrollController {
    /// 创建并启动一个滚动会话。
    pub fn start(
        window: WindowIdentity,
        region: ScrollRegion,
        direction: ScrollDirection,
        first_page: PageSnapshot,
        config: ScrollControllerConfig,
    ) -> Result<Self, VisionError> {
        let config = config.validate()?;
        let mut session = ScrollSession::new(window, region, direction)?;
        session.start(first_page)?;
        Ok(Self {
            session,
            end_detector: ScrollEndDetector::new(config.end),
            config,
            accumulated_shift_px: 0.0,
        })
    }

    /// 返回按当前 EMA 计算的下一批滚轮输入。
    pub fn next_input(&self) -> Option<WheelSteps> {
        self.session.next_batch(self.accumulated_shift_px)
    }

    /// 观察一次滚轮后的稳定页，并给出下一步控制决定。
    pub fn observe_page(
        &mut self,
        steps: WheelSteps,
        page: PageSnapshot,
        actual_shift_px: f32,
    ) -> Result<ScrollControllerOutcome, VisionError> {
        let previous = self
            .session
            .current_page
            .as_ref()
            .ok_or_else(|| VisionError::Protocol {
                message: "scroll controller has no current page".to_owned(),
            })?;
        self.session.validate_page(&page)?;
        if self.end_detector.observe(previous, &page, actual_shift_px) {
            return Ok(ScrollControllerOutcome::EndOfScroll);
        }

        self.session.observe_displacement(steps, actual_shift_px);
        let transition = self.session.evaluate_page(&page, actual_shift_px)?;
        match &transition {
            PageTransition::Accepted { .. } => {
                let accepted = self.session.accept_page(page, &transition)?;
                self.accumulated_shift_px = 0.0;
                Ok(ScrollControllerOutcome::Accepted(accepted))
            }
            PageTransition::Undershot { actual_shift_px } => {
                self.accumulated_shift_px += *actual_shift_px;
                let next = self.next_input().ok_or(VisionError::ScrollNoMovement)?;
                Ok(ScrollControllerOutcome::Continue(next))
            }
            PageTransition::Overshot { .. } => {
                self.session
                    .record_recovery(self.config.max_recovery_attempts)?;
                self.accumulated_shift_px = 0.0;
                let recovery = reverse_steps(steps, self.session.calibration.max_batch)
                    .ok_or(VisionError::ScrollOvershot)?;
                Ok(ScrollControllerOutcome::Recover(recovery))
            }
            PageTransition::ContinuityUnproven { .. } => Err(VisionError::ScrollContentMutated),
            PageTransition::NoMovement => Err(VisionError::ScrollNoMovement),
        }
    }
}

/// 构造不超过校准上限的反向恢复输入。
fn reverse_steps(steps: WheelSteps, max_batch: u32) -> Option<WheelSteps> {
    let maximum = max_batch.min(i32::MAX as u32).max(1);
    let magnitude = steps.magnitude().min(maximum).max(1);
    let sign = if steps.get() > 0 { -1 } else { 1 };
    WheelSteps::new(sign * magnitude as i32)
}
