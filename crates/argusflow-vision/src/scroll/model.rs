//! 滚动会话使用的页面、锚点和输入校准领域模型。

use std::collections::BTreeMap;

use argusflow_core::WindowIdentity;
use serde::{Deserialize, Serialize};

use crate::{
    error::VisionError,
    frame::{FrameId, PhysicalRect},
    scene::{SceneId, VisualNodeId, VisualScene},
};

/// 滚动内容移动的逻辑方向；不把 Windows wheel 的正负号泄漏到业务层。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    /// 内容向上移动，通常由向下滚轮产生。
    Down,
    /// 内容向下移动，通常由向上滚轮产生。
    Up,
}

impl ScrollDirection {
    /// 返回 Windows `mouseData` 所需的单步符号。
    pub const fn wheel_sign(self) -> i32 {
        match self {
            Self::Down => -1,
            Self::Up => 1,
        }
    }
}

/// 一批离散滚轮步数；零步不构成有效输入批次。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WheelSteps(i32);

impl WheelSteps {
    /// 从非零的 Windows wheel 步数构造批次。
    pub const fn new(value: i32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    /// 返回带方向的步数。
    pub const fn get(self) -> i32 {
        self.0
    }

    /// 返回不带方向的步数。
    pub fn magnitude(self) -> u32 {
        self.0.unsigned_abs()
    }
}

/// 当前窗口滚动区域的视觉坐标范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScrollRegion {
    /// 区域在捕获帧中的物理像素范围。
    pub bounds: PhysicalRect,
}

impl ScrollRegion {
    /// 创建一个有效滚动区域。
    pub const fn new(bounds: PhysicalRect) -> Self {
        Self { bounds }
    }
}

/// 同一列表/聊天区域内的滚轮到像素位移校准。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrollCalibration {
    /// 每个 wheel step 的实际内容位移 EMA，单位为物理像素。
    pub pixels_per_wheel_step_ema: Option<f32>,
    /// 目标页面之间保留的 overlap 比例。
    pub preferred_overlap_ratio: f32,
    /// 单个输入批次的最大绝对步数。
    pub max_batch: u32,
    /// 认为页面确实移动的最小像素位移。
    pub min_shift_px: f32,
}

impl Default for ScrollCalibration {
    fn default() -> Self {
        Self {
            pixels_per_wheel_step_ema: None,
            preferred_overlap_ratio: 0.18,
            max_batch: 12,
            min_shift_px: 2.0,
        }
    }
}

impl ScrollCalibration {
    /// 校验滚动控制参数，拒绝会导致无限滚动或反向控制的配置。
    pub fn validate(&self) -> Result<(), VisionError> {
        if !(0.0..0.5).contains(&self.preferred_overlap_ratio)
            || self.max_batch == 0
            || self.max_batch > i32::MAX as u32
            || !(self.min_shift_px.is_finite() && self.min_shift_px > 0.0)
            || self
                .pixels_per_wheel_step_ema
                .is_some_and(|value| !(value.is_finite() && value > 0.0))
        {
            return Err(VisionError::Protocol {
                message: "invalid scroll calibration".to_owned(),
            });
        }
        Ok(())
    }

    /// 计算指定 viewport 高度下的目标内容位移。
    pub fn target_shift(&self, viewport_height: u32) -> f32 {
        viewport_height as f32 * (1.0 - self.preferred_overlap_ratio)
    }

    /// 根据当前 EMA 估算一个有上限的 wheel 输入批次。
    pub fn estimate_batch(
        &self,
        direction: ScrollDirection,
        remaining_shift_px: f32,
    ) -> Option<WheelSteps> {
        if !remaining_shift_px.is_finite() || remaining_shift_px <= 0.0 {
            return None;
        }
        let per_step = self.pixels_per_wheel_step_ema.unwrap_or(80.0).max(1.0);
        let magnitude = (remaining_shift_px / per_step)
            .ceil()
            .clamp(1.0, self.max_batch as f32) as i32;
        WheelSteps::new(magnitude * direction.wheel_sign())
    }

    /// 用一次实际位移更新 wheel 到像素的 EMA。
    pub fn update(&mut self, steps: WheelSteps, actual_shift_px: f32) {
        if actual_shift_px.is_finite() && actual_shift_px > 0.0 {
            let sample = actual_shift_px / steps.magnitude().max(1) as f32;
            self.pixels_per_wheel_step_ema = Some(match self.pixels_per_wheel_step_ema {
                Some(previous) => previous * 0.65 + sample * 0.35,
                None => sample,
            });
        }
    }
}

/// 页面底部用于证明前后页连续性的视觉锚点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrollAnchor {
    /// 锚点来源 node ID。
    pub node_id: VisualNodeId,
    /// 规范化文本；纯图片锚点可以为空。
    pub text: Option<String>,
    /// 锚点 bbox。
    pub bbox: PhysicalRect,
    /// 与位置无关的局部 patch/text 摘要。
    pub patch_hash: u64,
    /// 当前页内的近似唯一性，范围为 0 到 1。
    pub uniqueness: f32,
}

/// 页面内可参与 history 去重的内容项。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageItem {
    /// 来源 node ID，仅用于 Explain，不作为跨页身份。
    pub node_id: VisualNodeId,
    /// 规范化文本。
    pub text: String,
    /// 当前页中的物理位置。
    pub bbox: PhysicalRect,
    /// OCR 置信度。
    pub confidence: f32,
    /// 位置无关的 patch/text 摘要。
    pub patch_hash: u64,
    /// 用于 overlap 去重的内容签名。
    pub signature: u64,
}

/// 一份稳定 viewport page snapshot；只描述当前页，不承载整个滚动历史。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageSnapshot {
    /// 页面绑定的窗口身份。
    pub window: WindowIdentity,
    /// 产生页面的 scene ID。
    pub scene_id: SceneId,
    /// 产生页面的 frame ID。
    pub frame_id: FrameId,
    /// 页面对应的滚动区域。
    pub region: ScrollRegion,
    /// 页面内容签名，用于底部不动检测。
    pub content_signature: u64,
    /// 当前页的内容项。
    pub items: Vec<PageItem>,
    /// 当前页底部优先选择的锚点。
    pub anchors: Vec<ScrollAnchor>,
}

impl PageSnapshot {
    /// 从当前稳定 scene 建立页面快照并选择底部锚点。
    pub fn from_scene(scene: &VisualScene, region: ScrollRegion) -> Result<Self, VisionError> {
        if !scene.viewport.intersects(region.bounds) {
            return Err(VisionError::Protocol {
                message: "scroll region does not intersect current viewport".to_owned(),
            });
        }
        let mut items = scene
            .nodes
            .iter()
            .filter(|node| node.bbox.intersects(region.bounds))
            .map(|node| {
                let patch_hash = position_independent_hash(
                    &node.normalized_text,
                    node.bbox.width,
                    node.bbox.height,
                );
                PageItem {
                    node_id: node.id,
                    text: node.normalized_text.clone(),
                    bbox: node.bbox,
                    confidence: node.confidence,
                    patch_hash,
                    signature: content_signature(&node.normalized_text, patch_hash),
                }
            })
            .collect::<Vec<_>>();
        items.sort_by_key(|item| (item.bbox.y, item.bbox.x, item.node_id));
        let content_signature = page_signature(&items);
        let anchors = select_anchors(scene, region, &items);
        Ok(Self {
            window: scene.window,
            scene_id: scene.scene_id,
            frame_id: scene.frame_id,
            region,
            content_signature,
            items,
            anchors,
        })
    }
}

/// 前后页面锚点匹配的可解释证据。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorMatchEvidence {
    /// 匹配到的锚点对数量。
    pub matched: usize,
    /// 文本相等的锚点数量。
    pub text_matches: usize,
    /// patch/text 摘要相等的锚点数量。
    pub patch_matches: usize,
}

/// 在旧页和新页之间进行确定性锚点匹配。
pub fn match_anchors(old: &PageSnapshot, new: &PageSnapshot) -> AnchorMatchEvidence {
    let mut evidence = AnchorMatchEvidence {
        matched: 0,
        text_matches: 0,
        patch_matches: 0,
    };
    let new_candidates = new
        .items
        .iter()
        .map(|item| ScrollAnchor {
            node_id: item.node_id,
            text: (!item.text.is_empty()).then_some(item.text.clone()),
            bbox: item.bbox,
            patch_hash: item.patch_hash,
            uniqueness: 1.0,
        })
        .collect::<Vec<_>>();
    let mut used_new = vec![false; new_candidates.len()];
    for old_anchor in &old.anchors {
        let Some((index, text_match, patch_match)) = new_candidates
            .iter()
            .enumerate()
            .filter(|(index, _)| !used_new[*index])
            .filter_map(|(index, new_anchor)| {
                let text_match = old_anchor.text.is_some() && old_anchor.text == new_anchor.text;
                let patch_match = old_anchor.patch_hash == new_anchor.patch_hash;
                (text_match || patch_match).then_some((index, text_match, patch_match))
            })
            .max_by_key(|(_, text_match, patch_match)| {
                (
                    u8::from(*text_match) + u8::from(*patch_match),
                    *text_match as u8,
                )
            })
        else {
            continue;
        };
        used_new[index] = true;
        evidence.matched += 1;
        evidence.text_matches += usize::from(text_match);
        evidence.patch_matches += usize::from(patch_match);
    }
    evidence
}

/// 选择页面底部 60% 到 90% 区间内的长文本、唯一且高置信度节点。
fn select_anchors(
    scene: &VisualScene,
    region: ScrollRegion,
    items: &[PageItem],
) -> Vec<ScrollAnchor> {
    let mut frequencies = BTreeMap::<&str, usize>::new();
    for item in items {
        *frequencies.entry(&item.text).or_default() += 1;
    }
    let mut candidates = scene
        .nodes
        .iter()
        .filter(|node| node.bbox.intersects(region.bounds))
        .filter_map(|node| {
            let relative_y =
                (node.center().1 - region.bounds.y as f32) / region.bounds.height as f32;
            if !(0.60..=0.90).contains(&relative_y) {
                return None;
            }
            let normalized = crate::normalize_text(&node.raw_text);
            let patch_hash =
                position_independent_hash(&normalized, node.bbox.width, node.bbox.height);
            let uniqueness =
                1.0 / frequencies.get(normalized.as_str()).copied().unwrap_or(1) as f32;
            Some(ScrollAnchor {
                node_id: node.id,
                text: (!normalized.is_empty()).then_some(normalized),
                bbox: node.bbox,
                patch_hash,
                uniqueness,
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .uniqueness
            .total_cmp(&left.uniqueness)
            .then_with(|| {
                right
                    .text
                    .as_ref()
                    .map_or(0, String::len)
                    .cmp(&left.text.as_ref().map_or(0, String::len))
            })
            .then_with(|| right.bbox.y.cmp(&left.bbox.y))
            .then_with(|| left.node_id.cmp(&right.node_id))
    });
    candidates.truncate(4);
    candidates.sort_by_key(|anchor| (anchor.bbox.y, anchor.bbox.x));
    candidates
}

/// 计算不依赖屏幕位置的轻量 patch/text 摘要。
fn position_independent_hash(text: &str, width: u32, height: u32) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text
        .as_bytes()
        .iter()
        .copied()
        .chain(width.to_le_bytes())
        .chain(height.to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 组合文本和 patch 摘要，形成 history overlap 的内容键。
fn content_signature(text: &str, patch_hash: u64) -> u64 {
    position_independent_hash(text, patch_hash as u32, (patch_hash >> 32) as u32)
}

/// 对页面内容项排序后计算稳定签名。
fn page_signature(items: &[PageItem]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for item in items {
        for byte in item.signature.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}
