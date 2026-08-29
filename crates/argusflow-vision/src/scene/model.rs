//! OCR 响应到单窗口 Scene 的完整构建与 Dirty ROI 合并。

use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use argusflow_core::{ScreenPoint, WindowIdentity};
use serde::{Deserialize, Serialize};

use crate::{CapturedFrame, OcrModel, OcrResponse, PhysicalRect, VisionError};

use super::{VisualNode, VisualNodeSource};

/// Runtime 单调分配的 Scene ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SceneId(u64);

impl SceneId {
    /// 创建 Scene ID。
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// 返回底层值。
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Scene 增量构建需要的最小上下文。
#[derive(Debug, Clone, Default)]
pub struct SceneBuildOptions {
    /// 局部刷新时保留未相交节点的上一份 Scene。
    pub base_scene: Option<Arc<VisualScene>>,
    /// 本次 OCR 实际覆盖的 Dirty ROI。
    pub refresh_regions: Vec<PhysicalRect>,
}

/// 不包含敏感文本的 OCR 执行摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneOcrSummary {
    /// 实际使用的 Small/Medium 档位。
    pub models: Vec<OcrModel>,
    /// OCR ROI 请求数量。
    pub request_count: usize,
    /// OCR 文本框数量。
    pub item_count: usize,
    /// Worker 报告的总耗时。
    pub elapsed_ms: u64,
    /// 实际执行增强的 ROI 数量。
    pub enhanced_request_count: usize,
    /// 最大几何放大比例千分值。
    pub max_scale_milli: u32,
}

impl SceneOcrSummary {
    /// 汇总同一帧的一组 OCR 响应。
    fn from_responses(responses: &[OcrResponse]) -> Self {
        let mut models = Vec::new();
        let mut item_count = 0_usize;
        let mut elapsed_ms = 0_u64;
        let mut enhanced_request_count = 0_usize;
        let mut max_scale_milli = 1_000_u32;
        for response in responses {
            if !models.contains(&response.model) {
                models.push(response.model);
            }
            item_count = item_count.saturating_add(response.items.len());
            elapsed_ms = elapsed_ms.saturating_add(response.elapsed_ms);
            enhanced_request_count += usize::from(response.preprocessing.was_applied());
            max_scale_milli = max_scale_milli.max(response.preprocessing.scale_milli());
        }
        Self {
            models,
            request_count: responses.len(),
            item_count,
            elapsed_ms,
            enhanced_request_count,
            max_scale_milli,
        }
    }
}

/// 一个 HWND 最近一次稳定、可查询的 OCR Scene。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualScene {
    /// 单调 Scene ID。
    pub scene_id: SceneId,
    /// 捕获 Frame ID。
    pub frame_id: crate::FrameId,
    /// 窗口移动/resize Generation。
    pub topology_generation: crate::TopologyGeneration,
    /// HWND/PID 身份。
    pub window: WindowIdentity,
    /// 帧本地物理范围。
    pub viewport: PhysicalRect,
    /// 帧左上角对应的虚拟屏幕坐标。
    pub viewport_origin: ScreenPoint,
    /// 按 y/x 排序的 OCR 节点。
    pub nodes: Vec<VisualNode>,
    /// 本次 OCR 摘要。
    pub ocr: SceneOcrSummary,
    /// 构建时间，仅用于诊断 freshness。
    pub built_at_unix_ms: u64,
}

/// 负责单调 Scene ID 和增量节点合并。
#[derive(Debug, Default)]
pub struct VisualSceneBuilder {
    /// 下一个 Scene ID。
    next_scene_id: u64,
}

impl VisualSceneBuilder {
    /// 创建从 Scene 1 开始的 Builder。
    pub fn new() -> Self {
        Self { next_scene_id: 1 }
    }

    /// 从同一帧 OCR 响应构建完整或局部更新 Scene。
    pub fn build(
        &mut self,
        window: WindowIdentity,
        frame: &CapturedFrame,
        responses: &[OcrResponse],
        options: &SceneBuildOptions,
    ) -> Result<Arc<VisualScene>, VisionError> {
        if frame.window != window || responses.is_empty() {
            return Err(VisionError::OcrFailed {
                message: "Scene build requires a matching frame and at least one OCR response"
                    .to_owned(),
            });
        }
        if responses.iter().any(|response| {
            response.frame_id != frame.frame_id
                || response.topology_generation != frame.topology_generation
        }) {
            return Err(VisionError::OcrCancelled {
                reason: "OCR response no longer belongs to the active frame".to_owned(),
            });
        }
        let scene_id = SceneId::new(self.next_scene_id);
        self.next_scene_id = self.next_scene_id.saturating_add(1);
        let mut nodes = Vec::new();
        for response in responses {
            let source = match response.model {
                OcrModel::PpOcrV6Small => VisualNodeSource::OcrSmall,
                OcrModel::PpOcrV6Medium => VisualNodeSource::OcrMedium,
            };
            for item in &response.items {
                if let Some(node) = VisualNode::from_ocr(
                    scene_id,
                    item.raw_text.clone(),
                    item.confidence,
                    item.polygon.clone(),
                    source,
                ) {
                    merge_node(&mut nodes, node);
                }
            }
        }
        if let Some(base) = &options.base_scene
            && base.window == window
            && base.topology_generation == frame.topology_generation
        {
            for node in &base.nodes {
                if !options
                    .refresh_regions
                    .iter()
                    .any(|region| node.bbox.intersects(*region))
                {
                    merge_node(&mut nodes, node.clone());
                }
            }
        }
        nodes.sort_by_key(|node| (node.bbox.y, node.bbox.x, node.id));
        Ok(Arc::new(VisualScene {
            scene_id,
            frame_id: frame.frame_id,
            topology_generation: frame.topology_generation,
            window,
            viewport: frame.bounds(),
            viewport_origin: frame.screen_origin(),
            nodes,
            ocr: SceneOcrSummary::from_responses(responses),
            built_at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_millis() as u64,
        }))
    }
}

/// 去除重叠 ROI 的重复节点，Medium 或更高置信度优先。
fn merge_node(nodes: &mut Vec<VisualNode>, candidate: VisualNode) {
    let duplicate = nodes.iter().position(|node| {
        node.normalized_text == candidate.normalized_text && node.bbox.intersects(candidate.bbox)
    });
    if let Some(index) = duplicate {
        let existing = &nodes[index];
        let better_model = matches!(candidate.source, VisualNodeSource::OcrMedium)
            && matches!(existing.source, VisualNodeSource::OcrSmall);
        if better_model
            || (candidate.source == existing.source && candidate.confidence > existing.confidence)
        {
            nodes[index] = candidate;
        }
    } else {
        nodes.push(candidate);
    }
}

#[cfg(test)]
mod tests {
    use argusflow_core::WindowIdentity;

    use super::*;
    use crate::{
        FrameId, OcrItem, OcrPreprocessingSummary, OcrRequestId, OcrTimingSummary, PolygonPoint,
        QpcTimestamp, TopologyGeneration,
    };

    #[test]
    fn dirty_roi_replaces_only_intersecting_nodes() {
        let window = WindowIdentity {
            handle: 7,
            process_id: 11,
        };
        let first_frame = frame(window, 1);
        let mut builder = VisualSceneBuilder::new();
        let first = builder
            .build(
                window,
                &first_frame,
                &[response(
                    &first_frame,
                    OcrModel::PpOcrV6Small,
                    vec![item("旧值", 10.0, 10.0), item("保持", 70.0, 70.0)],
                )],
                &SceneBuildOptions::default(),
            )
            .expect("initial full scene should build");
        let retained_generation = first.nodes[1].generation;
        let next_frame = frame(window, 2);
        let refresh_region = PhysicalRect::new(0, 0, 40, 40).expect("valid dirty ROI");

        let next = builder
            .build(
                window,
                &next_frame,
                &[response(
                    &next_frame,
                    OcrModel::PpOcrV6Small,
                    vec![item("新值", 12.0, 12.0)],
                )],
                &SceneBuildOptions {
                    base_scene: Some(first),
                    refresh_regions: vec![refresh_region],
                },
            )
            .expect("partial scene should merge");

        assert_eq!(next.nodes.len(), 2);
        assert!(next.nodes.iter().any(|node| node.raw_text == "新值"));
        assert!(!next.nodes.iter().any(|node| node.raw_text == "旧值"));
        assert_eq!(
            next.nodes
                .iter()
                .find(|node| node.raw_text == "保持")
                .expect("outside node should be retained")
                .generation,
            retained_generation,
        );
    }

    fn frame(window: WindowIdentity, id: u64) -> CapturedFrame {
        CapturedFrame::from_bgra8(
            FrameId::new(id),
            TopologyGeneration::new(1),
            window,
            QpcTimestamp::new(id),
            100,
            100,
            96,
            96,
            400,
            vec![0; 100 * 100 * 4],
        )
        .expect("fixture frame should be valid")
    }

    fn response(frame: &CapturedFrame, model: OcrModel, items: Vec<OcrItem>) -> OcrResponse {
        OcrResponse {
            request_id: OcrRequestId::new(),
            frame_id: frame.frame_id,
            topology_generation: frame.topology_generation,
            model,
            elapsed_ms: 1,
            preprocessing: OcrPreprocessingSummary {
                input_width: frame.width,
                input_height: frame.height,
                output_width: frame.width,
                output_height: frame.height,
                contrast_enhanced: false,
                sharpened: false,
                binarized: false,
            },
            timings: OcrTimingSummary {
                preprocess_elapsed_ms: 0,
                inference_elapsed_ms: 1,
            },
            model_input: None,
            items,
        }
    }

    fn item(text: &str, x: f32, y: f32) -> OcrItem {
        OcrItem {
            raw_text: text.to_owned(),
            confidence: 0.99,
            polygon: vec![
                PolygonPoint { x, y },
                PolygonPoint { x: x + 10.0, y },
                PolygonPoint {
                    x: x + 10.0,
                    y: y + 10.0,
                },
                PolygonPoint { x, y: y + 10.0 },
            ],
        }
    }
}
