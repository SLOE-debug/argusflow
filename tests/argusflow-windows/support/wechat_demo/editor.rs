//! 输入区空状态：用像素排除被 OCR 识别成 I 的绿色细光标，不忽略真实文字。
use super::{frame::Frame, snapshot::Observation};
use argusflow_capture_contracts::PixelRect;

/// 任意非光标文字框仍阻止发送或清空确认，不能仅忽略字符串 I、1 或竖线。
pub fn is_empty(snapshot: &Observation, region: PixelRect) -> bool {
    snapshot
        .blocks
        .iter()
        .filter(|b| region.intersection(b.rect).is_some())
        .all(|b| is_caret(&snapshot.frame, b.rect))
}

fn is_caret(frame: &Frame, rect: PixelRect) -> bool {
    if rect.width() > 20 || rect.height() > frame.bounds.height() / 15 || rect.height() < 8 {
        return false;
    }
    let width = frame.bounds.width() as usize;
    let first = (rect.y() as usize * width + rect.x() as usize) * 3;
    let background = &frame.pixels[first..first + 3];
    let mut ink = 0;
    let mut min_x = rect.right();
    let mut max_x = rect.x();
    for y in rect.y()..rect.bottom() {
        for x in rect.x()..rect.right() {
            let offset = (y as usize * width + x as usize) * 3;
            let pixel = &frame.pixels[offset..offset + 3];
            if (0..3).all(|c| pixel[c].abs_diff(background[c]) < 30) {
                continue;
            }
            let [b, g, r] = [pixel[0], pixel[1], pixel[2]];
            if g <= 100 || g.saturating_sub(r) <= 30 || g.saturating_sub(b) <= 20 {
                return false;
            }
            ink += 1;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
        }
    }
    ink >= 8 && max_x.saturating_sub(min_x) <= 3
}
