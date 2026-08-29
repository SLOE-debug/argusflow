//! VisualScene 构造和 current viewport 事实模型。

use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use argusflow_core::{ScreenPoint, WindowIdentity};
use serde::{Deserialize, Serialize};

use crate::{
    error::VisionError,
    frame::{FrameId, PhysicalRect, TopologyGeneration},
    image::CapturedFrame,
    layout::{RowConfig, VisualLine, VisualRow, cluster_lines, cluster_rows},
    ocr::{OcrModel, OcrResponse, OcrSource},
    projection::{ProjectionOptions, compact_text, spatial_text},
};

use super::{VisualNode, VisualRegion, VisualRegionId, VisualRegionKind};

/// VisualScene 的单调逻辑 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SceneId(u64);

impl SceneId {
    /// 创建 scene ID。
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// 返回 scene ID 数值。
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Scene 构造阶段使用的布局弱先验和文本投影设置。
#[derive(Debug, Clone, PartialEq)]
pub struct SceneBuildOptions {
    /// 默认 OCR 节点所属区域类型。
    pub region_kind: VisualRegionKind,
    /// 是否输出 region marker 以及空间列数。
    pub projection: ProjectionOptions,
    /// 列表/消息 row 的几何聚类参数。
    pub row: RowConfig,
    /// ROI 局部 OCR 时需要保留的上一份 scene；全帧 OCR 时为空。
    pub base_scene: Option<Arc<VisualScene>>,
    /// 本次 OCR 覆盖的帧本地区域集合；区域外节点可从 base scene 保留。
    pub refresh_regions: Vec<PhysicalRect>,
}

impl Default for SceneBuildOptions {
    fn default() -> Self {
        Self {
            region_kind: VisualRegionKind::Content,
            projection: ProjectionOptions::default(),
            row: RowConfig::default(),
            base_scene: None,
            refresh_regions: Vec::new(),
        }
    }
}

/// 构建当前场景所使用的 OCR 请求摘要，不包含识别文字本身。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneOcrSummary {
    /// 实际参与构建的模型，按首次出现顺序去重。
    pub models: Vec<OcrModel>,
    /// 合并进场景的 OCR 响应数量。
    pub request_count: usize,
    /// OCR worker 返回的原始文字段数量。
    pub item_count: usize,
    /// 所有 OCR 响应报告的处理耗时总和，单位为毫秒。
    pub elapsed_ms: u64,
    /// 实际改变 OCR 输入像素的请求数量。
    pub enhanced_request_count: usize,
    /// 所有响应中的最大图像放大比例，按千分值存储。
    pub max_scale_milli: u32,
}

impl SceneOcrSummary {
    /// 从同一场景的响应集合生成不含敏感文字的执行摘要。
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
            if response.preprocessing.was_applied() {
                enhanced_request_count = enhanced_request_count.saturating_add(1);
            }
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

/// 当前窗口可见内容的完整视觉事实快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualScene {
    /// 单调 scene ID。
    pub scene_id: SceneId,
    /// 产生该 scene 的帧 ID。
    pub frame_id: FrameId,
    /// 产生该 scene 的窗口拓扑代数。
    pub topology_generation: TopologyGeneration,
    /// 绑定的 HWND/PID 身份。
    pub window: WindowIdentity,
    /// 当前 viewport 在帧中的物理像素范围。
    pub viewport: PhysicalRect,
    /// 当前 viewport 左上角在 virtual screen 中的物理坐标。
    pub viewport_origin: ScreenPoint,
    /// 场景区域。
    pub regions: Vec<VisualRegion>,
    /// 场景内全部视觉节点。
    pub nodes: Vec<VisualNode>,
    /// 产生当前视觉事实的 OCR 模型、响应数和耗时摘要。
    pub ocr: SceneOcrSummary,
    /// 视觉行聚类。
    pub lines: Vec<VisualLine>,
    /// 联系人/消息 row 聚类。
    pub rows: Vec<VisualRow>,
    /// 面向日志、规则和模型的紧凑投影。
    pub compact_text: String,
    /// 面向 inspector/debug 的空间投影。
    pub spatial_text: String,
    /// 构建时的 Unix 毫秒时间，仅用于 freshness 诊断。
    pub built_at_unix_ms: u64,
}

/// 负责从一个或多个 OCR ROI 响应构造新 current scene。
#[derive(Debug)]
pub struct VisualSceneBuilder {
    /// 下一个 scene ID；只在 runtime 内递增。
    next_scene_id: u64,
}

impl Default for VisualSceneBuilder {
    fn default() -> Self {
        Self { next_scene_id: 1 }
    }
}

impl VisualSceneBuilder {
    /// 创建从 scene 1 开始的 builder。
    pub fn new() -> Self {
        Self::default()
    }

    /// 合并同一帧的 OCR 响应，去除重叠 ROI 的重复 node。
    pub fn build(
        &mut self,
        window: WindowIdentity,
        frame: &CapturedFrame,
        responses: &[OcrResponse],
        options: &SceneBuildOptions,
    ) -> Result<Arc<VisualScene>, VisionError> {
        if frame.window != window {
            return Err(VisionError::WindowIdentityChanged {
                expected: window,
                actual: Some(frame.window),
            });
        }
        if responses.is_empty() {
            return Err(VisionError::OcrFailed {
                message: "cannot build a scene without OCR responses".to_owned(),
            });
        }
        for response in responses {
            if response.frame_id != frame.frame_id
                || response.topology_generation != frame.topology_generation
            {
                return Err(VisionError::OcrCancelled {
                    reason: "OCR response generation is older than the current frame".to_owned(),
                });
            }
        }
        let scene_id = SceneId::new(self.next_scene_id);
        self.next_scene_id = self.next_scene_id.saturating_add(1);
        let region_id = VisualRegionId::new(1);
        let mut nodes = Vec::new();
        for response in responses {
            let source = OcrSource::from(response.model);
            for item in &response.items {
                let Some(node) = VisualNode::from_ocr(
                    scene_id,
                    item.raw_text.clone(),
                    item.confidence,
                    item.polygon.clone(),
                    source,
                    Some(region_id),
                ) else {
                    continue;
                };
                merge_node(&mut nodes, node);
            }
        }
        if let Some(base_scene) = &options.base_scene {
            if base_scene.window == window
                && (base_scene.topology_generation.is_unknown()
                    || base_scene.topology_generation == frame.topology_generation)
            {
                for node in &base_scene.nodes {
                    let refreshed = options
                        .refresh_regions
                        .iter()
                        .any(|region| node.bbox.intersects(*region));
                    if options.refresh_regions.is_empty() || !refreshed {
                        merge_node(&mut nodes, node.clone());
                    }
                }
            }
        }
        nodes.sort_by_key(|node| (node.bbox.y, node.bbox.x, node.stable_hash));
        for node in &mut nodes {
            node.generation = scene_id;
            node.line_id = None;
            node.row_id = None;
        }
        let lines = cluster_lines(&mut nodes);
        let rows = cluster_rows(&mut nodes, options.row);
        let mut region = VisualRegion::new(region_id, options.region_kind, frame.bounds());
        region.node_ids = nodes.iter().map(|node| node.id).collect();
        let mut scene = VisualScene {
            scene_id,
            frame_id: frame.frame_id,
            topology_generation: frame.topology_generation,
            window,
            viewport: frame.bounds(),
            viewport_origin: frame.screen_origin(),
            regions: vec![region],
            nodes,
            ocr: SceneOcrSummary::from_responses(responses),
            lines,
            rows,
            compact_text: String::new(),
            spatial_text: String::new(),
            built_at_unix_ms: unix_time_ms(),
        };
        scene.compact_text = compact_text(&scene, &options.projection);
        scene.spatial_text = spatial_text(&scene, &options.projection);
        Ok(Arc::new(scene))
    }
}

/// 合并同一 ROI overlap 造成的重复文本；更高档模型或更高置信度结果优先。
fn merge_node(nodes: &mut Vec<VisualNode>, candidate: VisualNode) {
    let duplicate = nodes.iter().position(|existing| {
        existing.normalized_text == candidate.normalized_text
            && existing.bbox.intersects(candidate.bbox)
    });
    if let Some(index) = duplicate {
        if source_quality(candidate.source) > source_quality(nodes[index].source)
            || (source_quality(candidate.source) == source_quality(nodes[index].source)
                && candidate.confidence > nodes[index].confidence)
        {
            nodes[index] = candidate;
        }
    } else {
        nodes.push(candidate);
    }
}

/// 返回 OCR provenance 的稳定质量等级；布局和外部投影不抢占 OCR 原始事实。
const fn source_quality(source: super::node::VisualNodeSource) -> u8 {
    match source {
        super::node::VisualNodeSource::OcrTiny => 1,
        super::node::VisualNodeSource::OcrSmall => 2,
        super::node::VisualNodeSource::OcrMedium => 3,
        super::node::VisualNodeSource::LayoutHeuristic
        | super::node::VisualNodeSource::UiaProjection
        | super::node::VisualNodeSource::GuiGrounding => 0,
    }
}

/// 获取不依赖外部时钟格式的构建时间。
fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}
