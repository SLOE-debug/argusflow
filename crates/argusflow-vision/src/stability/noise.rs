//! 周期性动画、caret 和闪烁提示的短期时序噪声屏蔽。

use std::time::{Duration, Instant};

use crate::{diff::DirtyMap, error::VisionError, frame::PhysicalRect, image::CapturedFrame};

/// 时序噪声屏蔽参数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemporalNoiseConfig {
    /// 未再次观测到的候选模式保留时长。
    pub ttl: Duration,
    /// 同一 ROI 发生多少次反向切换后才允许屏蔽。
    pub min_reversals: u8,
    /// 单个 ROI 占帧面积超过该比例时不屏蔽，避免吞掉整页重排。
    pub max_region_ratio: f32,
}

impl Default for TemporalNoiseConfig {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(1),
            min_reversals: 1,
            max_region_ratio: 0.20,
        }
    }
}

impl TemporalNoiseConfig {
    /// 检查时序噪声配置。
    pub fn validate(self) -> Result<Self, VisionError> {
        if self.ttl.is_zero()
            || self.min_reversals == 0
            || !(0.0..=1.0).contains(&self.max_region_ratio)
        {
            return Err(VisionError::Protocol {
                message: "temporal noise configuration is invalid".to_owned(),
            });
        }
        Ok(self)
    }
}

/// 同一空间区域最近一次像素切换的摘要。
#[derive(Debug, Clone, Copy)]
struct NoiseEntry {
    /// 与差分结果对齐的帧本地区域。
    rect: PhysicalRect,
    /// 上一次切换的旧图案摘要。
    previous_fingerprint: u64,
    /// 上一次切换的新图案摘要。
    current_fingerprint: u64,
    /// 已确认的反向切换次数。
    reversals: u8,
    /// 最近一次更新时刻。
    last_seen: Instant,
}

/// 时序候选的上限，防止异常 dirty 流导致状态随时间增长。
const MAX_NOISE_ENTRIES: usize = 128;

/// 有界的短期时序噪声追踪器。
#[derive(Debug)]
pub struct TemporalNoiseMask {
    /// 噪声候选的过期和确认规则。
    config: TemporalNoiseConfig,
    /// 当前帧中数量很小的 dirty ROI 候选。
    entries: Vec<NoiseEntry>,
}

impl Default for TemporalNoiseMask {
    fn default() -> Self {
        Self {
            config: TemporalNoiseConfig::default(),
            entries: Vec::new(),
        }
    }
}

impl TemporalNoiseMask {
    /// 创建指定配置的时序噪声追踪器。
    pub fn new(config: TemporalNoiseConfig) -> Result<Self, VisionError> {
        Ok(Self {
            config: config.validate()?,
            entries: Vec::new(),
        })
    }

    /// 清空候选，供窗口或拓扑切换时重新建立时序证据。
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// 过滤可证明为周期性反向切换的 dirty ROI。
    ///
    /// 只要区域出现新的像素模式，候选就会被替换而不是继续屏蔽，因此新文本
    /// 或真正的布局变化不会因为曾经出现过闪烁而永久沉默。
    pub fn observe(
        &mut self,
        previous: &CapturedFrame,
        current: &CapturedFrame,
        dirty: &DirtyMap,
    ) -> Result<DirtyMap, VisionError> {
        self.observe_at(previous, current, dirty, Instant::now())
    }

    /// 在指定时刻执行一次观察，便于 deterministic 测试和 inspector 重放。
    pub fn observe_at(
        &mut self,
        previous: &CapturedFrame,
        current: &CapturedFrame,
        dirty: &DirtyMap,
        now: Instant,
    ) -> Result<DirtyMap, VisionError> {
        if previous.window != current.window
            || previous.width != current.width
            || previous.height != current.height
            || previous.pixel_format != current.pixel_format
        {
            return Err(VisionError::InvalidFrame {
                message: "temporal noise frames are not comparable".to_owned(),
            });
        }
        self.entries
            .retain(|entry| now.saturating_duration_since(entry.last_seen) <= self.config.ttl);
        if dirty.major_transition {
            self.clear();
            return Ok(dirty.clone());
        }

        let bounds = current.bounds();
        let mut retained_regions = Vec::with_capacity(dirty.regions.len());
        let mut masked_area = 0_u64;
        for region in &dirty.regions {
            let rect = region.rect;
            let is_candidate = rect.is_inside(bounds)
                && rect.area() as f32 / bounds.area().max(1) as f32 <= self.config.max_region_ratio;
            let masked = is_candidate && self.observe_region(previous, current, rect, now)?;
            if masked {
                masked_area = masked_area.saturating_add(rect.area());
            } else {
                retained_regions.push(*region);
            }
        }

        if masked_area == 0 {
            return Ok(dirty.clone());
        }
        let bounds_area = bounds.area().max(1);
        let unmasked_ratio = 1.0 - (masked_area.min(bounds_area) as f32 / bounds_area as f32);
        let mut filtered = dirty.clone();
        filtered.changed_area_ratio = dirty.changed_area_ratio * unmasked_ratio;
        filtered.regions = retained_regions;
        Ok(filtered)
    }

    /// 更新一个 ROI 的模式对，并返回它是否已经满足屏蔽阈值。
    fn observe_region(
        &mut self,
        previous: &CapturedFrame,
        current: &CapturedFrame,
        rect: PhysicalRect,
        now: Instant,
    ) -> Result<bool, VisionError> {
        let previous_fingerprint = fingerprint(previous, rect)?;
        let current_fingerprint = fingerprint(current, rect)?;
        let min_reversals = self.config.min_reversals;
        if previous_fingerprint == current_fingerprint {
            return Ok(false);
        }
        let Some(entry_index) = self.entries.iter().position(|entry| entry.rect == rect) else {
            if self.entries.len() >= MAX_NOISE_ENTRIES {
                let oldest = self
                    .entries
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, entry)| entry.last_seen)
                    .map(|(index, _)| index);
                if let Some(oldest) = oldest {
                    self.entries.swap_remove(oldest);
                }
            }
            self.entries.push(NoiseEntry {
                rect,
                previous_fingerprint,
                current_fingerprint,
                reversals: 0,
                last_seen: now,
            });
            return Ok(false);
        };
        let entry = &mut self.entries[entry_index];

        let reversed = previous_fingerprint == entry.current_fingerprint
            && current_fingerprint == entry.previous_fingerprint;
        if reversed {
            entry.reversals = entry.reversals.saturating_add(1);
        } else if previous_fingerprint != entry.previous_fingerprint
            || current_fingerprint != entry.current_fingerprint
        {
            entry.previous_fingerprint = previous_fingerprint;
            entry.current_fingerprint = current_fingerprint;
            entry.reversals = 0;
        }
        entry.last_seen = now;
        Ok(entry.reversals >= min_reversals)
    }
}

/// 对 ROI 的低密度亮度采样生成确定性摘要。
fn fingerprint(frame: &CapturedFrame, rect: PhysicalRect) -> Result<u64, VisionError> {
    if !rect.is_inside(frame.bounds()) {
        return Err(VisionError::InvalidRoi {
            rect,
            frame_id: frame.frame_id,
        });
    }
    let step_x = (rect.width / 8).max(1);
    let step_y = (rect.height / 8).max(1);
    let mut hash = 0xcbf29ce484222325_u64;
    for y in (rect.y as u32..rect.y as u32 + rect.height).step_by(step_y as usize) {
        for x in (rect.x as u32..rect.x as u32 + rect.width).step_by(step_x as usize) {
            let pixel = frame.pixel(x, y).ok_or_else(|| VisionError::InvalidFrame {
                message: "noise fingerprint sampled outside frame storage".to_owned(),
            })?;
            hash ^= u64::from(luma(pixel));
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    Ok(hash ^ (u64::from(rect.width) << 32) ^ u64::from(rect.height))
}

/// 将 BGRA 像素压缩成对颜色不敏感的亮度值。
fn luma(pixel: [u8; 4]) -> u8 {
    ((u16::from(pixel[0]) * 11 + u16::from(pixel[1]) * 59 + u16::from(pixel[2]) * 30) / 100) as u8
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use argusflow_core::WindowIdentity;

    use super::*;
    use crate::{
        diff::{DiffConfig, compute_dirty_map},
        frame::{FrameId, QpcTimestamp, TopologyGeneration},
    };

    fn frame(id: u64, patch_value: Option<u8>) -> CapturedFrame {
        let mut pixels = vec![0_u8; 64 * 64 * 4];
        if let Some(value) = patch_value {
            for y in 24..32 {
                for x in 24..32 {
                    let offset = (y * 64 + x) * 4;
                    pixels[offset..offset + 4].copy_from_slice(&[value; 4]);
                }
            }
        }
        CapturedFrame::from_bgra8(
            FrameId::new(id),
            TopologyGeneration::new(1),
            WindowIdentity {
                handle: 3,
                process_id: 4,
            },
            QpcTimestamp::new(id),
            64,
            64,
            96,
            96,
            64 * 4,
            Arc::<[u8]>::from(pixels),
        )
        .expect("fixture frame is valid")
    }

    #[test]
    fn alternating_roi_is_masked_after_reversal() {
        let old = frame(1, None);
        let current = frame(2, Some(255));
        let next = frame(3, None);
        let diff_config = DiffConfig {
            tile_size: 16,
            roi_padding_px: 0,
            ..DiffConfig::default()
        };
        let first = compute_dirty_map(Some(&old), &current, diff_config).expect("first diff works");
        let second =
            compute_dirty_map(Some(&current), &next, diff_config).expect("second diff works");
        let mut mask = TemporalNoiseMask::default();
        assert!(
            !mask
                .observe_at(&old, &current, &first, Instant::now())
                .expect("first observation")
                .regions
                .is_empty()
        );
        let filtered = mask
            .observe_at(&current, &next, &second, Instant::now())
            .expect("reversal observation");
        assert!(filtered.regions.is_empty());
        assert_eq!(filtered.changed_area_ratio, 0.0);
    }
}
