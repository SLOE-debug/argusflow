//! 视觉运行时的结构化错误；不把平台细节泄漏到核心动作契约。

use std::fmt;

use argusflow_core::WindowIdentity;
use thiserror::Error;

use crate::frame::{FrameId, PhysicalRect};

/// 一次场景刷新当前执行到的阶段，用于把端到端超时定位到具体边界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneExecutionPhase {
    /// 查询已有视觉场景缓存。
    CacheLookup,
    /// 打开或复用窗口捕获订阅。
    OpeningCapture,
    /// 等待窗口画面达到稳定条件。
    WaitingForStableFrame,
    /// 计算脏区域与本次刷新范围。
    PlanningRefresh,
    /// 从捕获帧裁剪 OCR 输入区域。
    PreparingOcrInput,
    /// 等待 OCR worker 返回识别结果。
    WaitingForWorker,
    /// 将 OCR 结果合并为视觉场景。
    MergingScene,
    /// 场景刷新已经完成。
    Completed,
}

impl SceneExecutionPhase {
    /// 返回适合日志和诊断元数据的稳定阶段名称。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CacheLookup => "cache_lookup",
            Self::OpeningCapture => "opening_capture",
            Self::WaitingForStableFrame => "waiting_for_stable_frame",
            Self::PlanningRefresh => "planning_refresh",
            Self::PreparingOcrInput => "preparing_ocr_input",
            Self::WaitingForWorker => "waiting_for_worker",
            Self::MergingScene => "merging_scene",
            Self::Completed => "completed",
        }
    }
}

impl fmt::Display for SceneExecutionPhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

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
    /// 场景刷新超过端到端截止时间，并保留了最后执行阶段与失败现场位置。
    #[error("visual scene timed out after {timeout_ms}ms in phase {phase}: {diagnostic}")]
    SceneTimeout {
        /// 场景刷新使用的总毫秒预算。
        timeout_ms: u64,
        /// 超时时任务最后进入的执行阶段。
        phase: SceneExecutionPhase,
        /// OCR 输入是否产生以及诊断文件保存位置。
        diagnostic: String,
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
