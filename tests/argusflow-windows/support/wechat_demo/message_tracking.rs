//! 有序气泡的短时数据关联：双向唯一最近邻、位移约束及新生轨迹。
//! 不用文本当 ID。重复周期完全混叠或轨迹断裂时不能推断“发送失败”。
use super::scroll_motion;
use argusflow_capture_contracts::PixelRect;
use std::time::Duration;

#[derive(Clone)]
struct Track {
    rect: PixelRect,
    /// 基线气泡为 false；新生必须发生在当时全部既有气泡下方。
    born: bool,
    /// 连续可见帧数；避免把动画中的碎片当成消息。
    observations: u32,
}

/// 不确定原因是观测能力边界，不是消息发送失败。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Uncertainty {
    /// 捕获间隔过长，不能使用局部运动假设继续分配 ID。
    CaptureGap,
    /// 两个方向的匹配不唯一或气泡消失于视口内部。
    AmbiguousMotion,
    /// 没有观察到新气泡轨迹，可能是完全重复画面的混叠。
    NoBirth,
    /// 多条新生轨迹，无法归因到本次唯一 Enter。
    MultipleBirths,
}

/// 只保留上一帧几何；不缓存历史截图或执行 OCR。
pub struct Tracker {
    region: PixelRect,
    tracks: Vec<Track>,
    uncertainty: Option<Uncertainty>,
    births: u32,
}
impl Tracker {
    /// 初始化已有消息的身份，必须在按 Enter 之前调用。
    pub fn new(region: PixelRect, baseline: Vec<PixelRect>) -> Self {
        Self {
            region,
            tracks: baseline
                .into_iter()
                .map(|rect| Track {
                    rect,
                    born: false,
                    observations: 1,
                })
                .collect(),
            uncertainty: None,
            births: 0,
        }
    }

    /// 无唯一全局配准时，相邻采样限定 120ms、位移不超过间距 1/3 及 24px。
    /// 注册位移来自重叠图像的唯一低残差，不能由消息数量推算。
    pub fn observe(
        &mut self,
        boxes: Vec<PixelRect>,
        elapsed: Duration,
        registered_shift: Option<i32>,
    ) {
        if self.uncertainty.is_some() {
            return;
        }
        let old: Vec<_> = self.tracks.iter().map(|t| t.rect).collect();
        let shift = registered_shift.or_else(|| scroll_motion::estimate(&old, &boxes, self.region));
        if shift.is_none() && elapsed > Duration::from_millis(120) {
            self.uncertainty = Some(Uncertainty::CaptureGap);
            return;
        }
        let spacing = self
            .tracks
            .windows(2)
            .map(|p| p[1].rect.y().saturating_sub(p[0].rect.y()) / 3)
            .min()
            .unwrap_or(24)
            .clamp(3, 24);
        let mut next = Vec::new();
        let mut used = vec![false; self.tracks.len()];
        let predicted: Vec<_> = old
            .iter()
            .map(|r| scroll_motion::translated(*r, shift.unwrap_or(0), self.region))
            .collect();
        let tolerance = if shift.is_some() { 4 } else { spacing };
        let old_bottom = predicted.last().and_then(|r| r.map(|r| r.bottom()));
        for rect in boxes {
            let candidates: Vec<_> = self
                .tracks
                .iter()
                .enumerate()
                .filter(|(i, _)| predicted[*i].is_some_and(|p| compatible(p, rect, tolerance)))
                .map(|(i, _)| i)
                .collect();
            match candidates.as_slice() {
                [i] if !used[*i] => {
                    used[*i] = true;
                    let mut track = self.tracks[*i].clone();
                    track.rect = rect;
                    track.observations += 1;
                    next.push(track);
                }
                [] if old_bottom
                    .is_none_or(|bottom| rect.y() >= bottom.saturating_sub(spacing))
                    && rect.bottom() + 2 < self.region.bottom() =>
                {
                    self.births += 1;
                    next.push(Track {
                        rect,
                        born: true,
                        observations: 1,
                    });
                }
                [] if rect.bottom() + 2 >= self.region.bottom() => {
                    // 边界进入可能只是滚动到旧历史，不能发放新 ID。
                    self.uncertainty = Some(Uncertainty::AmbiguousMotion);
                    return;
                }
                _ => {
                    self.uncertainty = Some(Uncertainty::AmbiguousMotion);
                    return;
                }
            }
        }
        for ((track, predicted), matched) in self.tracks.iter().zip(predicted).zip(used) {
            // 只允许顶部离开视口；中间凭空消失意味着数据关联不成立。
            if !matched
                && predicted.is_some()
                && track.rect.bottom() > self.region.y() + spacing + 4
            {
                self.uncertainty = Some(Uncertainty::AmbiguousMotion);
                return;
            }
        }
        self.tracks = next;
    }

    /// 只有唯一新生且至少连续三帧可见的轨迹才能交给 OCR 验证。
    pub fn candidate(&self) -> Result<PixelRect, Uncertainty> {
        if let Some(reason) = self.uncertainty {
            return Err(reason);
        }
        if self.births > 1 {
            return Err(Uncertainty::MultipleBirths);
        }
        self.tracks
            .iter()
            .find(|t| t.born && t.observations >= 3)
            .map(|t| t.rect)
            .ok_or(Uncertainty::NoBirth)
    }
}

fn compatible(old: PixelRect, new: PixelRect, step: u32) -> bool {
    old.x().abs_diff(new.x()) <= 4
        && old.width().abs_diff(new.width()) <= 6
        && old.y().abs_diff(new.y()) <= step
        && old.height().abs_diff(new.height()) <= step
        && new.y() <= old.y() + 3
}
