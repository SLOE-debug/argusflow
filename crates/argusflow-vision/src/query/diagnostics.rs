//! 视觉查询的结构化 OCR 诊断与单行运行日志摘要。

use argusflow_core::VisualQuery;
use serde::{Deserialize, Serialize};

use crate::{
    frame::PhysicalRect,
    scene::{SceneOcrSummary, VisualNode, VisualNodeSource, VisualScene},
};

/// 单个匹配候选的可解释 OCR 事实。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualQueryCandidateSummary {
    /// OCR 返回的原始文字。
    pub raw_text: String,
    /// 帧本地物理像素位置。
    pub bbox: PhysicalRect,
    /// OCR 置信度，范围为 0 到 1。
    pub confidence: f32,
    /// 候选的视觉来源。
    pub source: VisualNodeSource,
}

/// 一次视觉查询对应的模型、耗时和匹配候选摘要。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualQueryReport {
    /// 查询的原始文字。
    pub query: String,
    /// 是否使用归一化后的精确匹配。
    pub exact: bool,
    /// 参与查询的场景 ID。
    pub scene_id: u64,
    /// 参与查询的捕获帧 ID。
    pub frame_id: u64,
    /// 该场景的 OCR 执行摘要。
    pub ocr: SceneOcrSummary,
    /// 查询命中的完整候选数量。
    pub candidate_count: usize,
    /// 查询命中的候选事实；运行日志展示时会限制数量和文字长度。
    pub candidates: Vec<VisualQueryCandidateSummary>,
}

impl VisualQueryReport {
    /// 从已经完成区域和文字过滤的候选集合构造报告。
    pub fn from_matches(
        scene: &VisualScene,
        query: &VisualQuery,
        candidates: &[&VisualNode],
    ) -> Self {
        Self {
            query: query.text.clone(),
            exact: query.exact,
            scene_id: scene.scene_id.get(),
            frame_id: scene.frame_id.get(),
            ocr: scene.ocr.clone(),
            candidate_count: candidates.len(),
            candidates: candidates
                .iter()
                .map(|node| VisualQueryCandidateSummary {
                    raw_text: node.raw_text.clone(),
                    bbox: node.bbox,
                    confidence: node.confidence,
                    source: node.source,
                })
                .collect(),
        }
    }

    /// 生成人类可读的单行日志，保留定位歧义所需的最小 OCR 事实。
    pub fn summary(&self) -> String {
        let model_names = self
            .ocr
            .models
            .iter()
            .map(|model| model.display_name())
            .collect::<Vec<_>>()
            .join(" + ");
        let model_names = if model_names.is_empty() {
            "OCR"
        } else {
            model_names.as_str()
        };
        let match_mode = if self.exact { "精确" } else { "包含" };
        let mut summary = format!(
            "{model_names} {match_mode}查询“{}”：命中 {}/{} 段，耗时 {} ms",
            compact_text(&self.query),
            self.candidate_count,
            self.ocr.item_count,
            self.ocr.elapsed_ms,
        );
        if self.ocr.enhanced_request_count > 0 {
            summary.push_str(&format!(
                "；增强 {}/{} 个 ROI，最高 {:.2}×",
                self.ocr.enhanced_request_count,
                self.ocr.request_count,
                f64::from(self.ocr.max_scale_milli) / 1_000.0,
            ));
        }
        for candidate in self.candidates.iter().take(4) {
            let bounds = candidate.bbox;
            let confidence = (candidate.confidence.clamp(0.0, 1.0) * 100.0).round() as u8;
            summary.push_str(&format!(
                "；“{}” [{},{},{}×{}] {confidence}%",
                compact_text(&candidate.raw_text),
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
            ));
        }
        if self.candidate_count > 4 {
            summary.push_str(&format!("；另有 {} 个候选", self.candidate_count - 4));
        }
        summary
    }
}

/// 把 OCR 多行文字压缩成日志安全的单行，并限制单个候选占用的长度。
fn compact_text(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = normalized.chars();
    let compact = characters.by_ref().take(48).collect::<String>();
    if characters.next().is_some() {
        format!("{compact}…")
    } else {
        compact
    }
}
