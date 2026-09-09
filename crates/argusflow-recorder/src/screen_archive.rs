//! 独立于输入事件的无损屏幕时间线契约。

use argusflow_core::{
    CaptureFailure, CaptureGeneration, CaptureRevision, CaptureSourceId, InspectionRect,
};
use serde::{Deserialize, Serialize};

/// 一场录制内唯一的屏幕帧标识；所有区域文件由该标识派生。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ScreenFrameId(pub u64);

/// 原始区域 PNG 的位置与范围；路径不可由 IPC 调用方指定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenPatch {
    /// 帧内顺序标识。
    pub index: u32,
    /// 虚拟屏幕物理像素范围。
    pub bounds: InspectionRect,
}

/// 一个已完成持久化的完整基准或增量更新。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenFrame {
    /// 场内唯一身份。
    pub id: ScreenFrameId,
    /// 显示输出身份。
    pub source: CaptureSourceId,
    /// 几何或设备代数。
    pub generation: CaptureGeneration,
    /// 来源内部变化序号。
    pub revision: CaptureRevision,
    /// 录制开始后的呈现微秒数。
    pub presented_us: u64,
    /// 录制开始后的冻结微秒数。
    pub frozen_us: u64,
    /// 完整输出的物理范围。
    pub bounds: InspectionRect,
    /// 完整基准没有前驱；增量只引用同来源同代数帧。
    pub previous: Option<ScreenFrameId>,
    /// Pending/Failed 时为原生候选范围，Complete 时为精确变化范围。
    pub changes: Vec<InspectionRect>,
    /// 成功编码的区域列表。
    pub patches: Vec<ScreenPatch>,
}

/// 归档完成事实；缺口或容量失败不得冒充完整录制。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ScreenCompleteness {
    /// 所有交付的变化均已持久化。
    #[default]
    Complete,
    /// 截图隐私策略显式关闭了屏幕归档。
    Disabled,
    /// 在首个不可恢复错误处停止，只发布有效前缀。
    Incomplete {
        /// 可穷尽处理的失败分类。
        reason: CaptureFailure,
    },
}

/// 无输入期间的变化也保存在此处。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScreenTimeline {
    /// 已录制的时间跨度；去重不能缩短静止尾段。
    pub duration_us: u64,
    /// 离线 GPU 精确化状态，与采集完整性独立。
    pub refinement: ScreenRefinement,
    /// 实际管线计数，不使用采样帧率推算漏帧。
    pub diagnostics: ScreenDiagnostics,
    /// 按归档顺序保存，呈现排序由 source 时间决定。
    pub frames: Vec<ScreenFrame>,
    /// 完整性状态。
    pub completeness: ScreenCompleteness,
}

/// 原始候选归档与精确归档不能混淆。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ScreenRefinement {
    /// 已保留候选像素，尚未完成 GPU 比较。
    #[default]
    Pending,
    /// GPU 已验证真实变化并重建依赖。
    Complete,
    /// 后处理失败，保留可重试的原始候选归档。
    Failed {
        /// 不含输入内容的后处理失败说明。
        message: String,
    },
}

/// 录制场内的资源峰值与差分成本。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScreenDiagnostics {
    /// 编码排队及处理中作业峰值，硬上限 64。
    pub queue_peak_items: usize,
    /// 未完成作业持有像素的保守字节峰值，硬上限 128MiB。
    pub queue_peak_bytes: usize,
    /// 原生读回的实际区域字节总量。
    pub readback_bytes: u64,
    /// 精确差分及快照更新的累计微秒。
    pub diff_total_us: u64,
}

/// 输入与画面的时间关联，不声明像素变化的因果关系。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EventScreenEvidence {
    /// 每个显示输出在输入之前的最新状态。
    pub before: Vec<ScreenFrameId>,
    /// 输入之后至下一操作锚点之间的全部变化。
    pub after: Vec<ScreenFrameId>,
}

/// 图像文件名只由已验证的帧与区域 ID 构造。
pub(crate) fn patch_name(frame: ScreenFrameId, patch: u32) -> String {
    format!("screen-{}-{patch}.png", frame.0)
}
