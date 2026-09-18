//! 稀疏几何的平移配准：枚举位移假设，选择有唯一多数支持的全局滚动。
//! 对周期相同的气泡不做大位移配准，避免跨一个周期错认消息身份。
use argusflow_capture_contracts::PixelRect;

/// 至少两个形状不同的旧气泡支持同一位移；平票或支持不足时拒绝估计。
pub fn estimate(old: &[PixelRect], new: &[PixelRect], viewport: PixelRect) -> Option<i32> {
    if old.len() < 2
        || !old
            .windows(2)
            .any(|p| p[0].width().abs_diff(p[1].width()) > 12)
    {
        return None;
    }
    let mut best: Option<(i32, usize)> = None;
    let mut tied = false;
    for a in old {
        for b in new {
            if a.width().abs_diff(b.width()) > 4 || a.height().abs_diff(b.height()) > 4 {
                continue;
            }
            let shift = b.y() as i32 - a.y() as i32;
            if shift > 3 || shift.unsigned_abs() > viewport.height() / 2 {
                continue;
            }
            let mut matches = 0;
            let mut valid = true;
            for r in old {
                if let Some(predicted) = translated(*r, shift, viewport) {
                    let hits = new.iter().filter(|n| close(predicted, **n)).count();
                    if hits != 1 {
                        valid = false;
                        break;
                    }
                    matches += 1;
                }
            }
            if !valid || matches < 2 {
                continue;
            }
            match best {
                None => {
                    best = Some((shift, matches));
                    tied = false;
                }
                Some((_, score)) if matches > score => {
                    best = Some((shift, matches));
                    tied = false;
                }
                Some((previous, score)) if matches == score && previous.abs_diff(shift) > 3 => {
                    tied = true
                }
                _ => {}
            }
        }
    }
    if tied {
        None
    } else {
        best.map(|(shift, _)| shift)
    }
}

/// 裁掉视口上方的部分，面积过小的边缘碎片不能继续作为轨迹。
pub fn translated(rect: PixelRect, shift: i32, viewport: PixelRect) -> Option<PixelRect> {
    let top = (i64::from(rect.y()) + i64::from(shift)).max(i64::from(viewport.y()));
    let bottom = (i64::from(rect.bottom()) + i64::from(shift)).min(i64::from(viewport.bottom()));
    if bottom - top < 13 {
        return None;
    }
    PixelRect::new(rect.x(), top as u32, rect.width(), (bottom - top) as u32).ok()
}

fn close(a: PixelRect, b: PixelRect) -> bool {
    a.x().abs_diff(b.x()) <= 4
        && a.y().abs_diff(b.y()) <= 3
        && a.width().abs_diff(b.width()) <= 4
        && a.height().abs_diff(b.height()) <= 4
}
