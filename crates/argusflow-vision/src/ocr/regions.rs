//! 变化像素生成完整行带，避免把一行文字切成两段；大范围变化直接全图检测。
use crate::{OcrResult, TextBlock};
use argusflow_image::ChangeRegion;

#[derive(Clone, Copy, Debug)]
pub(super) struct Band {
    pub top: u32,
    pub bottom: u32,
}
impl Band {
    pub fn intersects(self, block: &TextBlock) -> bool {
        let (top, bottom) = vertical(block);
        top < self.bottom as f32 && bottom > self.top as f32
    }
}

pub(super) fn plan(changes: &[ChangeRegion], previous: &OcrResult, height: u32) -> Vec<Band> {
    // 保留上下文供检测卷积使用；整行宽度使字符连接、空格和水平滚动不丢失上下文。
    let padding = 32;
    let mut bands: Vec<_> = changes
        .iter()
        .map(|change| Band {
            top: change.bounds.y().saturating_sub(padding),
            bottom: change.bounds.bottom().saturating_add(padding).min(height),
        })
        .collect();
    merge(&mut bands);
    // 若已有文字穿过裁剪边界，扩展到整块文字之外；合并后继续直到稳定。
    loop {
        let mut expanded = false;
        for band in &mut bands {
            for block in previous.blocks() {
                if band.intersects(block) {
                    let (top, bottom) = vertical(block);
                    let top = (top.max(0.0) as u32).saturating_sub(padding);
                    let bottom = (bottom.ceil().max(0.0) as u32)
                        .saturating_add(padding)
                        .min(height);
                    if top < band.top || bottom > band.bottom {
                        band.top = band.top.min(top);
                        band.bottom = band.bottom.max(bottom);
                        expanded = true;
                    }
                }
            }
        }
        merge(&mut bands);
        if !expanded {
            break;
        }
    }
    // 过多小任务与大面积滚动不适合增量；这是明确的算法策略，不是失败时静默回退。
    let coverage: u64 = bands.iter().map(|b| u64::from(b.bottom - b.top)).sum();
    if bands.len() > 8 || coverage * 5 >= u64::from(height) * 3 {
        vec![Band {
            top: 0,
            bottom: height,
        }]
    } else {
        bands
    }
}
fn merge(bands: &mut Vec<Band>) {
    bands.sort_by_key(|b| b.top);
    let mut merged: Vec<Band> = Vec::new();
    for band in bands.iter().copied() {
        if let Some(last) = merged.last_mut()
            && band.top <= last.bottom
        {
            last.bottom = last.bottom.max(band.bottom);
        } else {
            merged.push(band);
        }
    }
    *bands = merged;
}
fn vertical(block: &TextBlock) -> (f32, f32) {
    (
        block
            .polygon()
            .iter()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min),
        block
            .polygon()
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max),
    )
}
