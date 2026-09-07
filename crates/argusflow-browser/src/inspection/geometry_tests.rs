use super::geometry::{ViewportMetrics, ViewportTransform};
use argusflow_core::{InspectionContext, InspectionRect, ScreenPoint, WindowIdentity};

fn fixture(dpr: f64) -> (InspectionContext, ViewportMetrics) {
    (
        InspectionContext {
            window: WindowIdentity {
                handle: 7,
                process_id: 8,
            },
            executable_path: None,
            title: String::new(),
            class_name: String::new(),
            bounds: InspectionRect {
                x: -1920.0,
                y: -500.0,
                width: 1900.0,
                height: 1400.0,
            },
            browser_viewport: Some(InspectionRect {
                x: -1800.0,
                y: -400.0,
                width: 800.0 * dpr,
                height: 600.0 * dpr,
            }),
            dpi: 144,
            has_keyboard_focus: true,
        },
        ViewportMetrics {
            width: 800.0,
            height: 600.0,
            dpr,
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            focused: true,
            visible: true,
            url: "https://example.test/form".into(),
        },
    )
}

#[test]
fn physical_origin_is_not_scaled_under_mixed_dpi_and_browser_zoom() {
    for dpr in [1.0, 1.25, 1.5, 2.0, 2.5] {
        let (context, metrics) = fixture(dpr);
        let transform = ViewportTransform::new(&context, &metrics).unwrap();
        let physical = ScreenPoint {
            x: -1800 + (100.0 * dpr) as i32,
            y: -400 + (200.0 * dpr) as i32,
        };
        assert_eq!(transform.to_css(physical).unwrap(), (100, 200));
        let bounds = transform.screen_bounds(InspectionRect {
            x: 100.0,
            y: 200.0,
            width: 20.0,
            height: 10.0,
        });
        assert_eq!(bounds.x, f64::from(physical.x));
        assert_eq!(bounds.y, f64::from(physical.y));
    }
}

#[test]
fn toolbar_background_page_missing_renderer_and_pinch_are_rejected() {
    let (mut context, mut metrics) = fixture(1.5);
    let transform = ViewportTransform::new(&context, &metrics).unwrap();
    assert!(transform.to_css(ScreenPoint { x: -1700, y: -450 }).is_err());
    metrics.focused = false;
    assert!(ViewportTransform::new(&context, &metrics).is_err());
    metrics.focused = true;
    metrics.scale = 2.0;
    assert!(ViewportTransform::new(&context, &metrics).is_err());
    metrics.scale = 1.0;
    metrics.dpr = f64::NAN;
    assert!(ViewportTransform::new(&context, &metrics).is_err());
    metrics.dpr = 1.5;
    context.browser_viewport = None;
    assert!(ViewportTransform::new(&context, &metrics).is_err());
}
