//! 区域裁剪回归：窗口变化、跨屏坐标、局部像素和上下文边距。
use crate::evidence_region::*;
use argusflow_core::{EvidenceFrame, EvidencePixelFormat, InspectionRect, ScreenPoint};
use std::sync::Arc;

fn desktop(value: u8) -> Arc<EvidenceFrame> {
    Arc::new(
        EvidenceFrame::new(
            InspectionRect {
                x: -256.0,
                y: -64.0,
                width: 512.0,
                height: 256.0,
            },
            512,
            256,
            EvidencePixelFormat::Rgba8,
            vec![value; 512 * 256 * 4],
        )
        .unwrap(),
    )
}

#[test]
fn search_changes_crop_with_context_instead_of_whole_desktop() {
    let previous = desktop(0);
    let mut pixels = previous.pixels().to_vec();
    // 搜索框文字改变一个像素，原点在左侧显示器。
    let offset = (96 * 512 + 96) * 4;
    pixels[offset..offset + 4].copy_from_slice(&[9, 8, 7, 255]);
    let current = Arc::new(
        EvidenceFrame::new(previous.bounds(), 512, 256, previous.format(), pixels).unwrap(),
    );
    let scope = InspectionRect {
        x: -200.0,
        y: 0.0,
        width: 200.0,
        height: 160.0,
    };
    let mut bounds = None;
    for rect in changes(Some(&previous), &current)
        .into_iter()
        .filter_map(|rect| intersect(rect, scope))
    {
        include(&mut bounds, rect);
    }
    let result = crop(&current, scope, bounds, None).unwrap();
    assert_eq!(
        result.bounds(),
        InspectionRect {
            x: -184.0,
            y: 8.0,
            width: 49.0,
            height: 49.0
        }
    );
    assert_eq!(
        &result.pixels()[(24 * 49 + 24) * 4..(24 * 49 + 24) * 4 + 4],
        &[9, 8, 7, 255]
    );
}

#[test]
fn unrelated_window_changes_do_not_expand_scope_and_no_change_keeps_window() {
    let frame = desktop(42);
    let scope = InspectionRect {
        x: -200.0,
        y: 0.0,
        width: 200.0,
        height: 160.0,
    };
    let unrelated = InspectionRect {
        x: 100.0,
        y: 0.0,
        width: 50.0,
        height: 50.0,
    };
    assert!(intersect(scope, unrelated).is_none());
    let result = crop(&frame, scope, Some(unrelated), None).unwrap();
    assert_eq!(result.bounds(), scope);
    assert_eq!(result.width(), 200);
    assert_eq!(result.height(), 160);
}

#[test]
fn click_remains_visible_and_window_is_clipped_at_monitor_edge() {
    let frame = desktop(42);
    let scope = InspectionRect {
        x: -300.0,
        y: -100.0,
        width: 200.0,
        height: 200.0,
    };
    let point = ScreenPoint { x: -255, y: -63 };
    let change = InspectionRect {
        x: -160.0,
        y: 0.0,
        width: 32.0,
        height: 32.0,
    };
    let result = crop(&frame, scope, Some(change), Some(point)).unwrap();
    assert!(result.bounds().contains(point));
    assert_eq!(result.bounds().x, -256.0);
    assert_eq!(result.bounds().y, -64.0);
    assert!(result.width() < frame.width());
}
