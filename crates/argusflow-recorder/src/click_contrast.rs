//! 从点击邻域真实像素选择标记颜色；不修改原始 PNG。
use argusflow_core::{EvidenceFrame, ScreenPoint};

/// 选择与邻域最差亮度对比最大的颜色；排除红色以免混淆错误提示。
pub(crate) fn color(frame: &EvidenceFrame, point: ScreenPoint) -> Option<[u8; 3]> {
    if !frame.bounds().contains(point) {
        return None;
    }
    let x = (point.x as f64 - frame.bounds().x) as i32;
    let y = (point.y as f64 - frame.bounds().y) as i32;
    let mut samples = Vec::new();
    // 取点击周围 32px 的边框邻域，匹配预览中的定位框。
    for dy in (-18_i32..=18).step_by(3) {
        for dx in (-18_i32..=18).step_by(3) {
            let (sx, sy) = (x + dx, y + dy);
            if sx < 0 || sy < 0 || sx >= frame.width() as i32 || sy >= frame.height() as i32 {
                continue;
            }
            let offset = (sy as usize * frame.width() as usize + sx as usize) * 4;
            let pixel = &frame.pixels()[offset..offset + 3];
            samples.push(luminance([pixel[0], pixel[1], pixel[2]]));
        }
    }
    const COLORS: [[u8; 3]; 5] = [
        [0, 0, 0],
        [255, 255, 255],
        [0, 255, 255],
        [255, 255, 0],
        [0, 80, 255],
    ];
    COLORS
        .into_iter()
        .max_by(|a, b| score(*a, &samples).total_cmp(&score(*b, &samples)))
}

fn luminance(rgb: [u8; 3]) -> f64 {
    let linear = rgb.map(|channel| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    });
    linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
}

fn score(rgb: [u8; 3], samples: &[f64]) -> f64 {
    let foreground = luminance(rgb);
    samples
        .iter()
        .map(|&background| {
            (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
        })
        .fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use argusflow_core::{EvidencePixelFormat, InspectionRect};
    #[test]
    fn marker_adapts_to_dark_light_and_red_backgrounds() {
        for (background, expected) in [
            ([0, 0, 0, 255], [255, 255, 255]),
            ([255; 4], [0, 0, 0]),
            ([255, 0, 0, 255], [0, 0, 0]),
        ] {
            let frame = EvidenceFrame::new(
                InspectionRect {
                    x: -10.0,
                    y: -10.0,
                    width: 20.0,
                    height: 20.0,
                },
                20,
                20,
                EvidencePixelFormat::Rgba8,
                background.repeat(400),
            )
            .unwrap();
            assert_eq!(color(&frame, ScreenPoint { x: 0, y: 0 }), Some(expected));
            assert_eq!(color(&frame, ScreenPoint { x: -11, y: 0 }), None);
        }
    }
}
