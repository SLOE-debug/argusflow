//! Vision Runtime 到宿主 Run Artifact Store 的可选诊断边界。

use argusflow_core::RunTraceContext;
use serde::{Deserialize, Serialize};

use crate::{CapturedFrame, OcrRequest, OcrResponse, VisualQueryCandidateSummary};

/// 一次目标选择保持 0/1/N 与置信度门槛的诊断结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualSelectionOutcome {
    /// 查询没有合法候选。
    NotFound,
    /// 查询只有一个合法候选且通过门槛。
    Unique,
    /// 查询有多个合法候选，因此动作被阻止。
    Ambiguous,
    /// 唯一候选低于点击置信度门槛。
    RejectedConfidence,
}

/// Focus Mask 与 Root Cause 使用的查询/选择事实。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualQueryTrace {
    /// 参与查询的场景。
    pub scene_id: u64,
    /// 参与查询的捕获帧。
    pub frame_id: u64,
    /// AQL 或旧视觉查询源码。
    pub query: String,
    /// Runtime 0/1/N 与门槛结果。
    pub outcome: VisualSelectionOutcome,
    /// 合法候选或可获得的候选事实，顺序即 viewer Candidate 顺序。
    pub candidates: Vec<VisualQueryCandidateSummary>,
    /// 当前固定点击置信度门槛。
    pub minimum_click_confidence: f32,
    /// 唯一且通过门槛时为 0；其它结果为空。
    pub selected_candidate_index: Option<usize>,
    /// 选择阶段是否已阻止 SendInput。
    pub send_input_blocked: bool,
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

    /// 保存视觉查询候选、0/1/N 结果和 confidence gate。
    fn record_query(&self, context: &RunTraceContext, trace: &VisualQueryTrace);
}
