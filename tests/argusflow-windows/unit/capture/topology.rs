use super::*;
#[test]
fn rotated_rectangles_round_trip_at_every_edge() {
    for rotation in [
        Rotation::Identity,
        Rotation::Clockwise90,
        Rotation::Clockwise180,
        Rotation::Clockwise270,
    ] {
        for rect in [
            PixelRect::new(0, 0, 1, 1).unwrap(),
            PixelRect::new(63, 47, 1, 1).unwrap(),
            PixelRect::new(7, 9, 17, 22).unwrap(),
        ] {
            let logical = to_logical(rect, 64, 48, rotation).unwrap();
            assert_eq!(rect, to_raw(logical, 64, 48, rotation).unwrap());
        }
    }
}
#[test]
fn negative_screen_coordinates_preserve_physical_geometry() {
    let screen = ScreenRect::new(-1920, -400, 1920, 1080).unwrap();
    let crop = screen
        .project(PixelRect::new(10, 20, 30, 40).unwrap())
        .unwrap();
    assert_eq!(crop.x(), -1910);
    assert_eq!(crop.y(), -380);
    assert_eq!(crop.width(), 30);
}
