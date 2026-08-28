//! 基于锚点和条带 SAD 的滚动位移估算。

use serde::{Deserialize, Serialize};

use crate::{error::VisionError, frame::PhysicalRect, image::CapturedFrame};

use super::model::{PageSnapshot, ScrollRegion};

/// 位移估算使用的证据来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplacementMethod {
    /// 由跨页文本或 patch 锚点的几何位移得到。
    Anchor,
    /// 没有足够锚点时在滚动条带内搜索最小绝对差异。
    StripSad,
}

/// 位移搜索的采样和接受阈值。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DisplacementConfig {
    /// 允许搜索的最大绝对位移。
    pub search_limit_px: u32,
    /// 条带 SAD 的物理像素采样步长。
    pub sample_step_px: u32,
    /// 归一化 SAD 超过该值时视为没有可靠匹配。
    pub max_sad: f32,
}

impl Default for DisplacementConfig {
    fn default() -> Self {
        Self {
            search_limit_px: 512,
            sample_step_px: 4,
            max_sad: 0.22,
        }
    }
}

impl DisplacementConfig {
    /// 检查位移估算参数。
    pub fn validate(self) -> Result<Self, VisionError> {
        if self.search_limit_px == 0
            || self.search_limit_px > i32::MAX as u32
            || self.sample_step_px == 0
            || !(self.max_sad.is_finite() && self.max_sad > 0.0 && self.max_sad <= 1.0)
        {
            return Err(VisionError::Protocol {
                message: "displacement configuration is invalid".to_owned(),
            });
        }
        Ok(self)
    }
}

/// 一次滚动位移估算及其可解释置信度。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DisplacementEstimate {
    /// 内容沿垂直方向移动的绝对像素量。
    pub shift_y_px: f32,
    /// 由匹配覆盖率、锚点一致性或 SAD 余量构成的置信度。
    pub confidence: f32,
    /// 参与锚点估算的匹配数量。
    pub matched_anchors: usize,
    /// 产生估算的证据来源。
    pub method: DisplacementMethod,
}

/// 使用默认参数估算前后稳定页的垂直位移。
pub fn estimate_displacement(
    previous_frame: &CapturedFrame,
    current_frame: &CapturedFrame,
    previous_page: &PageSnapshot,
    current_page: &PageSnapshot,
    region: ScrollRegion,
) -> Result<DisplacementEstimate, VisionError> {
    estimate_displacement_with_config(
        previous_frame,
        current_frame,
        previous_page,
        current_page,
        region,
        DisplacementConfig::default(),
    )
}

/// 使用显式配置估算前后稳定页的垂直位移。
pub fn estimate_displacement_with_config(
    previous_frame: &CapturedFrame,
    current_frame: &CapturedFrame,
    previous_page: &PageSnapshot,
    current_page: &PageSnapshot,
    region: ScrollRegion,
    config: DisplacementConfig,
) -> Result<DisplacementEstimate, VisionError> {
    let config = config.validate()?;
    validate_inputs(
        previous_frame,
        current_frame,
        previous_page,
        current_page,
        region,
    )?;
    if let Some(estimate) = anchor_estimate(previous_page, current_page) {
        if estimate.shift_y_px >= 1.0 && estimate.confidence > 0.0 {
            return Ok(estimate);
        }
    }
    if previous_page.content_signature == current_page.content_signature
        && previous_page.anchors == current_page.anchors
    {
        return Err(VisionError::ScrollNoMovement);
    }
    strip_sad_estimate(previous_frame, current_frame, region.bounds, config)
        .ok_or(VisionError::ScrollNoMovement)
}

/// 校验位移比较不能跨越的身份和坐标边界。
fn validate_inputs(
    previous_frame: &CapturedFrame,
    current_frame: &CapturedFrame,
    previous_page: &PageSnapshot,
    current_page: &PageSnapshot,
    region: ScrollRegion,
) -> Result<(), VisionError> {
    if previous_frame.window != current_frame.window {
        return Err(VisionError::WindowIdentityChanged {
            expected: previous_frame.window,
            actual: Some(current_frame.window),
        });
    }
    if previous_frame.width != current_frame.width
        || previous_frame.height != current_frame.height
        || previous_frame.pixel_format != current_frame.pixel_format
    {
        return Err(VisionError::InvalidFrame {
            message: "displacement frames have incompatible dimensions".to_owned(),
        });
    }
    if previous_frame.topology_generation != current_frame.topology_generation
        && !previous_frame.topology_generation.is_unknown()
        && !current_frame.topology_generation.is_unknown()
    {
        return Err(VisionError::OcrCancelled {
            reason: "cannot estimate displacement across topology generations".to_owned(),
        });
    }
    if previous_page.window != previous_frame.window || current_page.window != current_frame.window
    {
        return Err(VisionError::WindowIdentityChanged {
            expected: previous_frame.window,
            actual: Some(current_page.window),
        });
    }
    if previous_page.region != region || current_page.region != region {
        return Err(VisionError::Protocol {
            message: "page and displacement region do not match".to_owned(),
        });
    }
    if !region.bounds.is_inside(previous_frame.bounds())
        || !region.bounds.is_inside(current_frame.bounds())
    {
        return Err(VisionError::InvalidRoi {
            rect: region.bounds,
            frame_id: current_frame.frame_id,
        });
    }
    Ok(())
}

/// 通过同文本或同 patch 的节点位置差计算锚点位移。
fn anchor_estimate(
    previous_page: &PageSnapshot,
    current_page: &PageSnapshot,
) -> Option<DisplacementEstimate> {
    let mut used = vec![false; current_page.items.len()];
    let mut shifts = Vec::new();
    for anchor in &previous_page.anchors {
        let Some((index, score)) = current_page
            .items
            .iter()
            .enumerate()
            .filter(|(index, _)| !used[*index])
            .filter_map(|(index, item)| {
                let text_match = anchor
                    .text
                    .as_ref()
                    .is_some_and(|text| !text.is_empty() && text == &item.text);
                let patch_match = anchor.patch_hash == item.patch_hash;
                (text_match || patch_match)
                    .then_some((index, u8::from(text_match) + u8::from(patch_match)))
            })
            .max_by_key(|(_, score)| *score)
        else {
            continue;
        };
        used[index] = true;
        if score == 0 {
            continue;
        }
        let shift = (anchor.bbox.y as f32 + anchor.bbox.height as f32 / 2.0
            - (current_page.items[index].bbox.y as f32
                + current_page.items[index].bbox.height as f32 / 2.0))
            .abs();
        if shift.is_finite() {
            shifts.push(shift);
        }
    }
    if shifts.is_empty() {
        return None;
    }
    shifts.sort_by(f32::total_cmp);
    let median = shifts[shifts.len() / 2];
    let spread = shifts
        .iter()
        .map(|shift| (*shift - median).abs())
        .sum::<f32>()
        / shifts.len() as f32;
    let coverage = shifts.len() as f32 / previous_page.anchors.len().max(1) as f32;
    let consistency = (1.0 - spread / median.max(1.0)).clamp(0.0, 1.0);
    Some(DisplacementEstimate {
        shift_y_px: median,
        confidence: (coverage * consistency).clamp(0.0, 1.0),
        matched_anchors: shifts.len(),
        method: DisplacementMethod::Anchor,
    })
}

/// 在正负两个方向搜索条带的最小平均绝对亮度差。
fn strip_sad_estimate(
    previous: &CapturedFrame,
    current: &CapturedFrame,
    region: PhysicalRect,
    config: DisplacementConfig,
) -> Option<DisplacementEstimate> {
    let max_shift = config.search_limit_px.min(region.height.saturating_sub(1));
    let mut best: Option<(f32, u32, f32)> = None;
    for shift in 1..=max_shift {
        for signed_shift in [shift as i32, -(shift as i32)] {
            let (score, texture) = strip_score(previous, current, region, signed_shift, config)?;
            if best.is_none_or(|(best_score, _, _)| score < best_score) {
                best = Some((score, shift, texture));
            }
        }
    }
    let (score, shift, texture) = best?;
    if score > config.max_sad || texture < 4.0 {
        return None;
    }
    Some(DisplacementEstimate {
        shift_y_px: shift as f32,
        confidence: ((1.0 - score / config.max_sad) * (texture / 32.0).min(1.0)).clamp(0.0, 1.0),
        matched_anchors: 0,
        method: DisplacementMethod::StripSad,
    })
}

/// 计算指定垂直偏移的采样 SAD 和当前条带亮度变化量。
fn strip_score(
    previous: &CapturedFrame,
    current: &CapturedFrame,
    region: PhysicalRect,
    signed_shift: i32,
    config: DisplacementConfig,
) -> Option<(f32, f32)> {
    // 只取内容区中央窄条带，避免把整块 OCR ROI 的 CPU 扫描成本带入控制环。
    let strip_width = region.width.min(64);
    let strip_x = i64::from(region.x) + i64::from(region.width - strip_width) / 2;
    let left = u32::try_from(strip_x).ok()?;
    let top = u32::try_from(region.y).ok()?;
    let right = left.checked_add(strip_width)?;
    let bottom = u32::try_from(region.bottom()).ok()?;
    let mut difference = 0_u64;
    let mut samples = 0_u64;
    let mut minimum = 255_u8;
    let mut maximum = 0_u8;
    for y in (top..bottom).step_by(config.sample_step_px as usize) {
        let shifted_y = i64::from(y) + i64::from(signed_shift);
        if shifted_y < i64::from(top) || shifted_y >= i64::from(bottom) {
            continue;
        }
        let shifted_y = u32::try_from(shifted_y).ok()?;
        for x in (left..right).step_by(config.sample_step_px as usize) {
            let old = luma(previous.pixel(x, shifted_y)?);
            let new = luma(current.pixel(x, y)?);
            difference += u64::from(old.abs_diff(new));
            samples += 1;
            minimum = minimum.min(new);
            maximum = maximum.max(new);
        }
    }
    if samples == 0 {
        return None;
    }
    Some((
        difference as f32 / samples as f32 / 255.0,
        f32::from(maximum.saturating_sub(minimum)),
    ))
}

/// 将 BGRA 像素压缩为亮度，降低颜色变化对位移匹配的干扰。
fn luma(pixel: [u8; 4]) -> u8 {
    ((u16::from(pixel[0]) * 11 + u16::from(pixel[1]) * 59 + u16::from(pixel[2]) * 30) / 100) as u8
}
