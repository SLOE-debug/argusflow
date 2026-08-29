//! VisualNode 事实模型和短期 stable identity。

use serde::{Deserialize, Serialize};

use crate::{
    frame::PhysicalRect,
    layout::{VisualLineId, VisualRowId},
    ocr::{OcrSource, PolygonPoint, normalize_text},
};

use super::{model::SceneId, region::VisualRegionId};

/// 单个 scene 内的逻辑节点 ID；不可作为跨重启 selector。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VisualNodeId(u64);

impl VisualNodeId {
    /// 由 deterministic stable hash 创建逻辑 ID。
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// 返回逻辑 ID 数值。
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// 对纯 OCR 节点的弱角色提示；角色不是 OCR 的事实字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleHint {
    /// 尚未推断角色。
    Unknown,
    /// 可能是导航或侧栏项目。
    NavigationItem,
    /// 可能是标题。
    Header,
    /// 可能是消息内容。
    Message,
    /// 可能是输入编辑器内容。
    Editor,
    /// 可能是弹出层内容。
    Popup,
}

/// OCR 文本节点的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualNodeSource {
    /// PP-OCRv6 tiny。
    OcrTiny,
    /// PP-OCRv6 medium。
    OcrMedium,
    /// 几何布局启发式。
    LayoutHeuristic,
    /// UIA 外层投影。
    UiaProjection,
    /// GUI grounding。
    GuiGrounding,
}

impl From<OcrSource> for VisualNodeSource {
    fn from(source: OcrSource) -> Self {
        match source {
            OcrSource::OcrTiny => Self::OcrTiny,
            OcrSource::OcrMedium => Self::OcrMedium,
            OcrSource::LayoutHeuristic => Self::LayoutHeuristic,
            OcrSource::UiaProjection => Self::UiaProjection,
            OcrSource::GuiGrounding => Self::GuiGrounding,
        }
    }
}

/// VisualScene 中的最小视觉事实单元。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualNode {
    /// 短期 logical stable ID。
    pub id: VisualNodeId,
    /// 产生该节点的 scene generation。
    pub generation: SceneId,
    /// OCR 原始文本。
    pub raw_text: String,
    /// 用于查询和投影的规范化文本。
    pub normalized_text: String,
    /// 弱角色提示，不替代真实语义属性。
    pub role_hint: RoleHint,
    /// 帧本地物理像素 bbox。
    pub bbox: PhysicalRect,
    /// 原始 OCR polygon。
    pub polygon: Vec<PolygonPoint>,
    /// OCR 置信度。
    pub confidence: f32,
    /// 来源 provenance。
    pub source: VisualNodeSource,
    /// 所属 region。
    pub region_id: Option<VisualRegionId>,
    /// 所属视觉行。
    pub line_id: Option<VisualLineId>,
    /// 所属列表/消息 row。
    pub row_id: Option<VisualRowId>,
    /// 用于相邻 scene 短期跟踪的确定性 patch/text hash。
    pub stable_hash: u64,
    /// 用于跨 scene 关联文本事实的 hash；不包含 bbox，允许同一文字移动后继续关联。
    pub identity_hash: u64,
}

impl VisualNode {
    /// 由 OCR item 构造 node，并统一计算 polygon bbox 和 normalized text。
    pub fn from_ocr(
        generation: SceneId,
        raw_text: String,
        confidence: f32,
        polygon: Vec<PolygonPoint>,
        source: OcrSource,
        region_id: Option<VisualRegionId>,
    ) -> Option<Self> {
        if !confidence.is_finite() {
            return None;
        }
        let bbox = polygon_bounds(&polygon)?;
        let normalized_text = normalize_text(&raw_text);
        if normalized_text.is_empty() {
            return None;
        }
        let stable_hash = stable_hash(&normalized_text, bbox, region_id);
        let identity_hash = identity_hash(&normalized_text, region_id);
        Some(Self {
            id: VisualNodeId::new(stable_hash),
            generation,
            raw_text,
            normalized_text,
            role_hint: RoleHint::Unknown,
            bbox,
            polygon,
            confidence: confidence.clamp(0.0, 1.0),
            source: source.into(),
            region_id,
            line_id: None,
            row_id: None,
            stable_hash,
            identity_hash,
        })
    }

    /// 返回 bbox 中心点。
    pub fn center(&self) -> (f32, f32) {
        (
            self.bbox.x as f32 + self.bbox.width as f32 / 2.0,
            self.bbox.y as f32 + self.bbox.height as f32 / 2.0,
        )
    }
}

/// 计算 OCR polygon 的包围盒。
fn polygon_bounds(polygon: &[PolygonPoint]) -> Option<PhysicalRect> {
    let first = polygon.first()?;
    let mut min_x = first.x.floor();
    let mut min_y = first.y.floor();
    let mut max_x = first.x.ceil();
    let mut max_y = first.y.ceil();
    for point in polygon.iter().skip(1) {
        min_x = min_x.min(point.x.floor());
        min_y = min_y.min(point.y.floor());
        max_x = max_x.max(point.x.ceil());
        max_y = max_y.max(point.y.ceil());
    }
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return None;
    }
    let x = min_x.max(0.0) as i32;
    let y = min_y.max(0.0) as i32;
    let right = max_x.max(min_x + 1.0) as i32;
    let bottom = max_y.max(min_y + 1.0) as i32;
    PhysicalRect::new(
        x,
        y,
        right.saturating_sub(x) as u32,
        bottom.saturating_sub(y) as u32,
    )
}

/// 轻量 deterministic FNV-1a hash，不把 DefaultHasher 当持久化协议。
fn stable_hash(text: &str, bbox: PhysicalRect, region_id: Option<VisualRegionId>) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    let region_value = region_id.map_or(0, VisualRegionId::get);
    for byte in text
        .as_bytes()
        .iter()
        .copied()
        .chain(bbox.x.to_le_bytes())
        .chain(bbox.y.to_le_bytes())
        .chain(bbox.width.to_le_bytes())
        .chain(bbox.height.to_le_bytes())
        .chain(region_value.to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 计算不依赖几何位置的文本事实身份，用于动作前后场景关联。
fn identity_hash(text: &str, region_id: Option<VisualRegionId>) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    let region_value = region_id.map_or(0, VisualRegionId::get);
    for byte in text
        .as_bytes()
        .iter()
        .copied()
        .chain(region_value.to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
