//! 微信默认主题的发出气泡检测；颜色规则属于应用适配，不属于通用 OCR。
use super::frame::Frame;
use argusflow_capture_contracts::PixelRect;

/// 扫描右侧绿色行带，不读取文字；头像和左侧收到的消息不参与跟踪。
pub fn outgoing(frame: &Frame, messages: PixelRect) -> Vec<PixelRect> {
    let left = messages.x() + messages.width() / 3;
    let right = messages.x() + messages.width() * 89 / 100;
    let mut bands = Vec::new();
    let mut band: Option<(u32, u32, u32, u32)> = None;
    for y in messages.y()..messages.bottom() {
        let mut xs = (left..right).filter(|&x| {
            let i = (y as usize * frame.bounds.width() as usize + x as usize) * 3;
            let [b, g, r] = [frame.pixels[i], frame.pixels[i + 1], frame.pixels[i + 2]];
            g > 100 && g.saturating_sub(r) > 30 && g.saturating_sub(b) > 20
        });
        let first = xs.next();
        let (count, last) = xs.fold((0, first.unwrap_or(left)), |(count, _), x| (count + 1, x));
        if let Some(x) = first.filter(|_| count >= 12) {
            band = Some(match band {
                Some((top, min_x, max_x, _)) => (top, min_x.min(x), max_x.max(last), y),
                None => (y, x, last, y),
            });
        } else if let Some((top, min_x, max_x, bottom)) = band.take()
            && bottom - top >= 12
            && let Ok(rect) = PixelRect::new(min_x, top, max_x - min_x + 1, bottom - top + 1)
        {
            bands.push(rect);
        }
    }
    // 保留触底部分气泡，让跟踪器识别它是边界进入而非完整新生。
    if let Some((top, min_x, max_x, bottom)) = band
        && bottom - top >= 12
        && let Ok(rect) = PixelRect::new(min_x, top, max_x - min_x + 1, bottom - top + 1)
    {
        bands.push(rect);
    }
    bands
}

/// 新气泡左侧状态槽必须接近聊天背景；红叹号、灰色转圈或未知图形都阻止确认。
/// 这是保守的视觉门槛，空槽不等于服务器回执。
pub fn status_slot_clear(frame: &Frame, bubble: PixelRect) -> bool {
    let left = bubble.x().saturating_sub(32);
    if left < 4 || bubble.y() < 2 {
        return false;
    }
    let offset = ((bubble.y() as usize - 2) * frame.bounds.width() as usize + left as usize) * 3;
    let background = &frame.pixels[offset..offset + 3];
    let mut different = 0;
    for y in bubble.y()..bubble.bottom() {
        for x in left..bubble.x().saturating_sub(5) {
            let i = (y as usize * frame.bounds.width() as usize + x as usize) * 3;
            if (0..3).any(|c| frame.pixels[i + c].abs_diff(background[c]) > 30) {
                different += 1;
            }
        }
    }
    different < 5
}
