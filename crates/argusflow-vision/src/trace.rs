//! Vision Runtime 到宿主 Run Artifact Store 的可选诊断边界。

use crate::{AppScene, CapturedFrame, OcrRequest, OcrResponse, VisionQueryMetrics, VisualNodeId};
use argusflow_core::RunTraceContext;

/// 一次视觉查询的严格选择结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisionSelectionOutcome {
    /// 完整场景中没有候选。
    NotFound,
    /// 唯一候选通过动作置信度门槛。
    Unique,
    /// 多项读取按阅读顺序返回多个候选。
    Multiple,
    /// 多候选或空间距离 tie 无法安全选择。
    Ambiguous,
    /// 唯一候选未通过动作置信度门槛。
    RejectedConfidence,
}

/// 宿主接收捕获帧、OCR ROI 与 exact model input 的 best-effort 接口。
pub trait VisionTraceSink: std::fmt::Debug + Send + Sync + 'static {
    /// 一次 OCR 响应完成后保存同源三层图像和结构化结果。
    fn record_ocr(
        &self,
        context: &RunTraceContext,
        frame: &CapturedFrame,
        request: &OcrRequest,
        response: &OcrResponse,
    );

    /// 保存一次 AQL 求值的完整 Scene 投影、选择结果和性能计数器。
    fn record_query(
        &self,
        context: &RunTraceContext,
        scene: &AppScene,
        query_source: &str,
        candidate_ids: &[VisualNodeId],
        selected_id: Option<VisualNodeId>,
        outcome: VisionSelectionOutcome,
        metrics: VisionQueryMetrics,
    );
}
