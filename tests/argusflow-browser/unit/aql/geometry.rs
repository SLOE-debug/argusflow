//! 嵌套 frame 的边框、缩放及无效变换。
use super::*;
#[test]
fn projects_scaled_frame_with_border_and_negative_origin() {
    let metrics = FrameMetrics {
        left: -10.0,
        top: 20.0,
        width: 400.0,
        height: 200.0,
        border_left: 2.0,
        border_top: 3.0,
        layout_width: 200.0,
        layout_height: 100.0,
        supported: true,
    };
    let point = project(CssPoint::new(10.0, 20.0).unwrap(), &metrics).unwrap();
    assert_eq!((point.x(), point.y()), (14.0, 66.0));
    let nested = project(point, &metrics).unwrap();
    assert_eq!((nested.x(), nested.y()), (22.0, 158.0));
}
#[test]
fn refuses_unrepresentable_or_missing_layout() {
    let metrics = FrameMetrics {
        left: 0.0,
        top: 0.0,
        width: 0.0,
        height: 200.0,
        border_left: 0.0,
        border_top: 0.0,
        layout_width: 100.0,
        layout_height: 100.0,
        supported: false,
    };
    assert!(project(CssPoint::new(0.0, 0.0).unwrap(), &metrics).is_err());
}
