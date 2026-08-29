//! WindowScene 中最小且可点击的 OCR 文本事实。

use serde::{Deserialize, Serialize};

use crate::{PhysicalRect, PolygonPoint, normalize_text};

use super::SceneId;

/// Scene 内由文本和几何确定的节点 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VisualNodeId(u64);

impl VisualNodeId {
    /// 从确定性 Hash 创建 ID。
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// 返回底层 ID。
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// 当前节点来自 Small 还是 Medium OCR。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualNodeSource {
    /// 默认 PP-OCRv6 Small。
    OcrSmall,
    /// 查询失败后的 PP-OCRv6 Medium 升级。
    OcrMedium,
}

/// 一个文本、Polygon、BBox 和置信度事实。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualNode {
    /// Scene 内稳定 ID。
    pub id: VisualNodeId,
    /// 最近一次刷新该节点的 Scene ID。
    pub generation: SceneId,
    /// PaddleOCR 原始文本。
    pub raw_text: String,
    /// 查询使用的规范化文本。
    pub normalized_text: String,
    /// 帧本地物理 BBox。
    pub bbox: PhysicalRect,
    /// PaddleOCR 原始 Polygon。
    pub polygon: Vec<PolygonPoint>,
    /// OCR 置信度。
    pub confidence: f32,
    /// OCR 模型来源。
    pub source: VisualNodeSource,
    /// 文本与 BBox Hash，用于 Dirty ROI 增量合并。
    pub stable_hash: u64,
    /// 不含位置的文本 Hash，用于短期动作前后比较。
    pub identity_hash: u64,
}

impl VisualNode {
    /// 从一个合法 OCR item 创建节点。
    pub fn from_ocr(
        generation: SceneId,
        raw_text: String,
        confidence: f32,
        polygon: Vec<PolygonPoint>,
        source: VisualNodeSource,
    ) -> Option<Self> {
        if !confidence.is_finite() {
            return None;
        }
        let bbox = polygon_bounds(&polygon)?;
        let normalized_text = normalize_text(&raw_text);
        if normalized_text.is_empty() {
            return None;
        }
        let stable_hash = hash_parts(&normalized_text, Some(bbox));
        let identity_hash = hash_parts(&normalized_text, None);
        Some(Self {
            id: VisualNodeId::new(stable_hash),
            generation,
            raw_text,
            normalized_text,
            bbox,
            polygon,
            confidence: confidence.clamp(0.0, 1.0),
            source,
            stable_hash,
            identity_hash,
        })
    }

    /// 返回 BBox 中心点。
    pub fn center(&self) -> (f32, f32) {
        (
            self.bbox.x as f32 + self.bbox.width as f32 / 2.0,
            self.bbox.y as f32 + self.bbox.height as f32 / 2.0,
        )
    }
}

/// 计算 Polygon 的非空帧本地 BBox。
fn polygon_bounds(polygon: &[PolygonPoint]) -> Option<PhysicalRect> {
    let first = polygon.first()?;
    let mut left = first.x;
    let mut top = first.y;
    let mut right = first.x;
    let mut bottom = first.y;
    for point in polygon.iter().skip(1) {
        left = left.min(point.x);
        top = top.min(point.y);
        right = right.max(point.x);
        bottom = bottom.max(point.y);
    }
    if ![left, top, right, bottom].into_iter().all(f32::is_finite) {
        return None;
    }
    let x = left.floor().max(0.0) as i32;
    let y = top.floor().max(0.0) as i32;
    let width = (right.ceil() as i64 - i64::from(x)).max(1) as u32;
    let height = (bottom.ceil() as i64 - i64::from(y)).max(1) as u32;
    PhysicalRect::new(x, y, width, height)
}

/// 使用稳定 FNV-1a 构造不依赖随机种子的节点 Hash。
fn hash_parts(text: &str, bbox: Option<PhysicalRect>) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text
        .as_bytes()
        .iter()
        .copied()
        .chain(bbox.into_iter().flat_map(|bbox| {
            bbox.x
                .to_le_bytes()
                .into_iter()
                .chain(bbox.y.to_le_bytes())
                .chain(bbox.width.to_le_bytes())
                .chain(bbox.height.to_le_bytes())
        }))
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
