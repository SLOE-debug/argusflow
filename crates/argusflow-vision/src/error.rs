//! 视觉运行时的结构化错误；不把平台细节泄漏到核心动作契约。

use argusflow_core::WindowIdentity;
use thiserror::Error;

use crate::frame::{FrameId, PhysicalRect};

/// 视觉捕获、OCR、场景和滚动管线的内部错误分类。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VisionError {
    /// 目标窗口当前不能建立捕获流。
    #[error("visual capture is unavailable: {message}")]
    CaptureUnavailable {
        /// 不含像素内容和用户文本的稳定错误摘要。
        message: String,
    },
    /// 捕获流返回的窗口身份已经不是 prepare 阶段绑定的身份。
    #[error("window identity changed from {expected:?} to {actual:?}")]
    WindowIdentityChanged {
        /// prepare 阶段冻结的窗口身份。
        expected: WindowIdentity,
        /// 当前重新读取到的窗口身份；窗口消失时为空。
        actual: Option<WindowIdentity>,
    },
    /// 在截止时间内没有得到新帧。
    #[error("visual frame timed out after {timeout_ms}ms")]
    FrameTimeout {
        /// 等待新帧使用的毫秒预算。
        timeout_ms: u64,
    },
    /// 得到的帧在截止时间内没有达到稳定条件。
    #[error("visual frame remained unstable after {observed_frames} frames and {timeout_ms}ms")]
    FrameUnstable {
        /// 已观察到的帧数。
        observed_frames: u32,
        /// 等待稳定使用的毫秒预算。
        timeout_ms: u64,
    },
    /// OCR worker 当前未就绪。
    #[error("visual worker is unavailable: {message}")]
    WorkerUnavailable {
        /// worker 状态或退出原因摘要。
        message: String,
    },
    /// OCR worker 返回了无法消费的结果。
    #[error("OCR failed: {message}")]
    OcrFailed {
        /// 不包含原始像素的错误摘要。
        message: String,
    },
    /// OCR 请求因帧或 generation 过期而被取消。
    #[error("OCR request was cancelled: {reason}")]
    OcrCancelled {
        /// 取消原因。
        reason: String,
    },
    /// 场景已经超过调用方允许的 freshness。
    #[error("visual scene is stale")]
    SceneStale,
    /// 视觉目标候选超过一个且没有显式选择规则。
    #[error("visual target is ambiguous: {matches} candidates")]
    AmbiguousVisualTarget {
        /// 候选数量。
        matches: usize,
    },
    /// 滚动实际位移超过目标区间。
    #[error("scroll overshot the requested page")]
    ScrollOvershot,
    /// 滚动输入没有造成可测位移。
    #[error("scroll produced no measurable movement")]
    ScrollNoMovement,
    /// 滚动期间内容发生了不能安全拼接的变化。
    #[error("scroll content mutated while crawling")]
    ScrollContentMutated,
    /// 视觉验证明确拒绝了动作后置条件。
    #[error("visual verification rejected the action: {reason}")]
    VerificationRejected {
        /// 面向用户和证据的拒绝原因。
        reason: String,
    },
    /// 视觉验证无法在截止时间内证明成功或失败。
    #[error("visual verification is uncertain: {reason}")]
    VerificationUncertain {
        /// 面向用户和证据的不确定原因。
        reason: String,
    },
    /// 捕获帧的宽高、步长或像素长度不满足不变量。
    #[error("invalid captured frame: {message}")]
    InvalidFrame {
        /// 无敏感数据的校验失败摘要。
        message: String,
    },
    /// ROI 不在当前帧坐标范围内。
    #[error("invalid ROI {rect:?} for frame {frame_id:?}")]
    InvalidRoi {
        /// 被拒绝的 ROI。
        rect: PhysicalRect,
        /// 关联帧。
        frame_id: FrameId,
    },
    /// worker 协议字段或版本不符合当前契约。
    #[error("invalid vision worker protocol: {message}")]
    Protocol {
        /// 不含 payload 的协议错误摘要。
        message: String,
    },
}
