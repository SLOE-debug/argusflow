//! 针对竖直聊天滚动的稀疏 SAD 配准；时间标签等纹理可打破同文气泡的周期歧义。
//! 只枚举气泡几何产生的位移假设，不引入 OpenCV/FFT 运行时；多峰时拒绝猜测。
use super::frame::Frame;
use argusflow_capture_contracts::PixelRect;
use std::collections::BTreeSet;

/// 比较有效重叠区并屏蔽共同背景；最佳残差必须显著低于第二个非邻近候选。
pub fn estimate(
    before: &Frame,
    after: &Frame,
    region: PixelRect,
    old: &[PixelRect],
    new: &[PixelRect],
) -> Option<i32> {
    if before.bounds != after.bounds || old.len() > 64 || new.len() > 64 {
        return None;
    }
    let mut shifts = BTreeSet::from([0]);
    for a in old {
        for b in new {
            let dy = b.y() as i32 - a.y() as i32;
            if dy <= 0
                && dy.unsigned_abs() <= region.height() / 2
                && a.width().abs_diff(b.width()) <= 5
            {
                shifts.insert(dy);
            }
        }
    }
    if shifts.len() > 64 {
        return None;
    }
    let mut scored: Vec<_> = shifts
        .into_iter()
        .filter_map(|dy| score(before, after, region, dy).map(|s| (dy, s)))
        .collect();
    scored.sort_by(|a, b| a.1.total_cmp(&b.1));
    let &(best, residual) = scored.first()?;
    let (_, runner_up) = scored.iter().find(|(shift, _)| shift.abs_diff(best) > 3)?;
    // 强度残差不是“发送成功概率”；没有次优假设不能声称唯一。
    if residual < 4.0 && *runner_up > residual * 2.0 + 0.5 {
        Some(best)
    } else {
        None
    }
}

fn score(before: &Frame, after: &Frame, region: PixelRect, shift: i32) -> Option<f64> {
    let width = before.bounds.width() as usize;
    let bg = (region.y() as usize * width + region.x() as usize) * 3;
    let background = &before.pixels[bg..bg + 3];
    let top = region.y() + shift.unsigned_abs();
    let mut residual = 0u64;
    let mut signal = 0u64;
    // 4×3 网格采样；候选通常远少于视口高度，避免逐像素穷举所有位移。
    for y in (top..region.bottom()).step_by(3) {
        let new_y = (y as i64 + i64::from(shift)) as usize;
        for x in (region.x() + 4..region.right().saturating_sub(4)).step_by(4) {
            let a = (y as usize * width + x as usize) * 3;
            let b = (new_y * width + x as usize) * 3;
            if (0..3).all(|c| {
                before.pixels[a + c].abs_diff(background[c]) < 25
                    && after.pixels[b + c].abs_diff(background[c]) < 25
            }) {
                continue;
            }
            signal += 1;
            residual += (0..3)
                .map(|c| u64::from(before.pixels[a + c].abs_diff(after.pixels[b + c])))
                .sum::<u64>();
        }
    }
    (signal >= 80).then(|| residual as f64 / (signal * 3) as f64)
}
