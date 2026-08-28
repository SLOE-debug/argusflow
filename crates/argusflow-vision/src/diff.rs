//! 低分辨率 tile 差分与 Dirty ROI 合并。

use serde::{Deserialize, Serialize};

use crate::{
    error::VisionError,
    frame::{FrameId, PhysicalRect},
    image::CapturedFrame,
};

/// 差分管线的可调参数；默认值对应实施方案中的第一版 benchmark seed。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiffConfig {
    /// 低分辨率采样比例，必须大于 0 且不超过 1。
    pub scale: f32,
    /// 逻辑 tile 的边长，单位为捕获像素。
    pub tile_size: u32,
    /// 单通道亮度差超过该值才算变化。
    pub pixel_threshold: u8,
    /// tile 内采样点达到该比例才把 tile 标成 dirty。
    pub tile_changed_ratio: f32,
    /// 全局变化超过该比例时直接升级为完整刷新。
    pub full_refresh_dirty_ratio: f32,
    /// 相邻 dirty tile 合并后向外扩展的像素数。
    pub roi_padding_px: u32,
    /// ROI 数量超过该值时合并成一个完整区域。
    pub max_regions: usize,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            scale: 0.25,
            tile_size: 32,
            pixel_threshold: 12,
            tile_changed_ratio: 0.08,
            full_refresh_dirty_ratio: 0.35,
            roi_padding_px: 16,
            max_regions: 32,
        }
    }
}

impl DiffConfig {
    /// 检查差分参数，避免无效配置导致除零或无限循环。
    pub fn validate(self) -> Result<Self, VisionError> {
        if !(self.scale > 0.0 && self.scale <= 1.0) {
            return Err(VisionError::Protocol {
                message: "diff scale must be in (0, 1]".to_owned(),
            });
        }
        if self.tile_size == 0 || self.max_regions == 0 {
            return Err(VisionError::Protocol {
                message: "diff tile size and max regions must be non-zero".to_owned(),
            });
        }
        if !(0.0..=1.0).contains(&self.tile_changed_ratio)
            || !(0.0..=1.0).contains(&self.full_refresh_dirty_ratio)
        {
            return Err(VisionError::Protocol {
                message: "diff ratios must be in [0, 1]".to_owned(),
            });
        }
        Ok(self)
    }

    /// 返回低分辨率采样之间的整数步长。
    fn sample_step(self) -> u32 {
        (1.0 / self.scale).ceil().max(1.0) as u32
    }
}

/// Dirty 区域产生的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirtyRegionReason {
    /// 没有可比较的前一稳定帧。
    InitialFrame,
    /// tile 像素差超过阈值。
    PixelDifference,
    /// 变化覆盖率过高，升级完整刷新。
    MajorTransition,
}

/// 一个已经 padding 且仍在帧边界内的 Dirty ROI。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DirtyRegion {
    /// 帧本地物理像素范围。
    pub rect: PhysicalRect,
    /// 未 padding 前该区域的变化比例。
    pub changed_ratio: f32,
    /// 区域进入 dirty map 的原因。
    pub reason: DirtyRegionReason,
}

/// 一次 current frame 相对于 previous frame 的结构化差分结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirtyMap {
    /// 被比较的当前帧。
    pub frame_id: FrameId,
    /// 未 padding 的全局变化比例。
    pub changed_area_ratio: f32,
    /// 实际采样点数量。
    pub compared_samples: u64,
    /// 超过像素阈值的采样点数量。
    pub changed_samples: u64,
    /// 是否应忽略局部 ROI，直接进行 full refresh。
    pub major_transition: bool,
    /// 合并后的 Dirty ROI。
    pub regions: Vec<DirtyRegion>,
}

impl DirtyMap {
    /// 判断一个查询区域是否与任何 dirty ROI 相交。
    pub fn intersects(&self, query: PhysicalRect) -> bool {
        self.regions
            .iter()
            .any(|region| region.rect.intersects(query))
    }
}

/// 计算两个同窗口、同尺寸 BGRA 帧之间的亮度 tile 差分。
pub fn compute_dirty_map(
    previous: Option<&CapturedFrame>,
    current: &CapturedFrame,
    config: DiffConfig,
) -> Result<DirtyMap, VisionError> {
    let config = config.validate()?;
    if let Some(previous) = previous {
        validate_comparable(previous, current)?;
    }
    let bounds = current.bounds();
    let Some(previous) = previous else {
        return Ok(DirtyMap {
            frame_id: current.frame_id,
            changed_area_ratio: 1.0,
            compared_samples: 0,
            changed_samples: 0,
            major_transition: true,
            regions: vec![DirtyRegion {
                rect: bounds,
                changed_ratio: 1.0,
                reason: DirtyRegionReason::InitialFrame,
            }],
        });
    };

    let sample_step = config.sample_step();
    let mut compared_samples = 0_u64;
    let mut changed_samples = 0_u64;
    let mut raw_regions = Vec::new();
    let mut y = 0_u32;
    while y < current.height {
        let tile_height = config.tile_size.min(current.height - y);
        let mut x = 0_u32;
        while x < current.width {
            let tile_width = config.tile_size.min(current.width - x);
            let mut tile_compared = 0_u64;
            let mut tile_changed = 0_u64;
            let mut sample_y = y;
            while sample_y < y + tile_height {
                let mut sample_x = x;
                while sample_x < x + tile_width {
                    let old = previous.pixel(sample_x, sample_y).ok_or_else(|| {
                        VisionError::InvalidFrame {
                            message: "previous frame pixel is outside its storage".to_owned(),
                        }
                    })?;
                    let new = current.pixel(sample_x, sample_y).ok_or_else(|| {
                        VisionError::InvalidFrame {
                            message: "current frame pixel is outside its storage".to_owned(),
                        }
                    })?;
                    let old_luma = luma(old);
                    let new_luma = luma(new);
                    tile_compared += 1;
                    compared_samples += 1;
                    if old_luma.abs_diff(new_luma) > config.pixel_threshold {
                        tile_changed += 1;
                        changed_samples += 1;
                    }
                    sample_x = sample_x.saturating_add(sample_step);
                }
                sample_y = sample_y.saturating_add(sample_step);
            }
            let tile_ratio = ratio(tile_changed, tile_compared);
            if tile_ratio >= config.tile_changed_ratio {
                raw_regions.push((
                    PhysicalRect::new(x as i32, y as i32, tile_width, tile_height).ok_or_else(
                        || VisionError::InvalidFrame {
                            message: "diff tile has zero area".to_owned(),
                        },
                    )?,
                    tile_ratio,
                ));
            }
            x = x.saturating_add(config.tile_size);
        }
        y = y.saturating_add(config.tile_size);
    }

    let changed_area_ratio = ratio(changed_samples, compared_samples);
    let major_transition = changed_area_ratio >= config.full_refresh_dirty_ratio;
    let regions = if major_transition || raw_regions.len() > config.max_regions {
        vec![DirtyRegion {
            rect: bounds,
            changed_ratio: changed_area_ratio,
            reason: DirtyRegionReason::MajorTransition,
        }]
    } else {
        merge_regions(raw_regions)
            .into_iter()
            .map(|(rect, changed_ratio)| DirtyRegion {
                rect: rect.expand_clamped(config.roi_padding_px, bounds),
                changed_ratio,
                reason: DirtyRegionReason::PixelDifference,
            })
            .collect()
    };
    Ok(DirtyMap {
        frame_id: current.frame_id,
        changed_area_ratio,
        compared_samples,
        changed_samples,
        major_transition,
        regions,
    })
}

/// 检查两个帧是否具有相同的窗口身份、尺寸和像素格式。
fn validate_comparable(
    previous: &CapturedFrame,
    current: &CapturedFrame,
) -> Result<(), VisionError> {
    if previous.window != current.window {
        return Err(VisionError::WindowIdentityChanged {
            expected: previous.window,
            actual: Some(current.window),
        });
    }
    if previous.width != current.width
        || previous.height != current.height
        || previous.pixel_format != current.pixel_format
    {
        return Err(VisionError::InvalidFrame {
            message: "frames have incompatible dimensions or pixel formats".to_owned(),
        });
    }
    Ok(())
}

/// 将 BGRA 四元组转换成无符号亮度，避免差分被单一色道放大。
fn luma(pixel: [u8; 4]) -> u8 {
    ((u16::from(pixel[0]) * 11 + u16::from(pixel[1]) * 59 + u16::from(pixel[2]) * 30) / 100) as u8
}

/// 对整数计数做安全比例计算。
fn ratio(numerator: u64, denominator: u64) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32
    }
}

/// 将共享边界的 tile 做确定性 connected merge。
fn merge_regions(mut regions: Vec<(PhysicalRect, f32)>) -> Vec<(PhysicalRect, f32)> {
    let mut index = 0;
    while index < regions.len() {
        let mut merged = false;
        let mut other_index = index + 1;
        while other_index < regions.len() {
            if regions[index].0.touches(regions[other_index].0) {
                let left = regions[index];
                let right = regions.remove(other_index);
                let area = left.0.area() + right.0.area();
                let weighted_ratio = if area == 0 {
                    0.0
                } else {
                    (left.1 * left.0.area() as f32 + right.1 * right.0.area() as f32) / area as f32
                };
                regions[index] = (left.0.union(right.0), weighted_ratio);
                merged = true;
                continue;
            }
            other_index += 1;
        }
        if !merged {
            index += 1;
        }
    }
    regions.sort_by_key(|(rect, _)| (rect.y, rect.x, rect.height, rect.width));
    regions
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use argusflow_core::WindowIdentity;

    use super::*;
    use crate::frame::{QpcTimestamp, TopologyGeneration};

    fn frame(frame_id: u64, fill: u8) -> CapturedFrame {
        CapturedFrame::from_bgra8(
            FrameId::new(frame_id),
            TopologyGeneration::new(0),
            WindowIdentity {
                handle: 7,
                process_id: 11,
            },
            QpcTimestamp::new(frame_id),
            64,
            64,
            96,
            96,
            64 * 4,
            Arc::<[u8]>::from(vec![fill; 64 * 64 * 4]),
        )
        .expect("fixture frame is valid")
    }

    #[test]
    fn first_frame_is_a_major_full_refresh() {
        let current = frame(1, 0);
        let map = compute_dirty_map(None, &current, DiffConfig::default()).expect("diff works");

        assert!(map.major_transition);
        assert_eq!(map.regions[0].rect, current.bounds());
    }

    #[test]
    fn unchanged_frame_has_no_dirty_regions() {
        let old = frame(1, 0);
        let current = frame(2, 0);
        let map =
            compute_dirty_map(Some(&old), &current, DiffConfig::default()).expect("diff works");

        assert_eq!(map.changed_samples, 0);
        assert!(map.regions.is_empty());
    }

    #[test]
    fn changed_tile_is_padded_and_clamped() {
        let old = frame(1, 0);
        let mut pixels = vec![0_u8; 64 * 64 * 4];
        for y in 32..64 {
            for x in 32..64 {
                let offset = (y * 64 + x) * 4;
                pixels[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let current = CapturedFrame::from_bgra8(
            FrameId::new(2),
            TopologyGeneration::new(0),
            old.window,
            QpcTimestamp::new(2),
            64,
            64,
            96,
            96,
            64 * 4,
            Arc::<[u8]>::from(pixels),
        )
        .expect("fixture frame is valid");
        let map =
            compute_dirty_map(Some(&old), &current, DiffConfig::default()).expect("diff works");

        assert_eq!(map.regions.len(), 1);
        assert!(map.regions[0].rect.is_inside(current.bounds()));
        assert_eq!(map.regions[0].rect.x, 16);
        assert_eq!(map.regions[0].rect.y, 16);
    }
}
