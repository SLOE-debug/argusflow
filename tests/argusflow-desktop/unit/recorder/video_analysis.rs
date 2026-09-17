use super::*;
fn thumbnail(rect: Option<[u32; 4]>, color: [u8; 3]) -> VideoThumbnail {
    let mut image = VideoThumbnail {
        width: 32,
        height: 20,
        pixels: vec![[80, 128, 128]; 640],
    };
    if let Some([x, y, w, h]) = rect {
        for row in y..y + h {
            for col in x..x + w {
                image.pixels[(row * 32 + col) as usize] = color;
            }
        }
    }
    image
}
#[test]
fn ignores_caret_and_detects_equal_luma_color_change() {
    let mut selector = Selection::new(thumbnail(None, [0; 3]), 1, 0);
    assert!(
        !selector
            .push(
                thumbnail(Some([3, 3, 1, 8]), [255, 128, 128]),
                2,
                100,
                400,
                0,
                1000
            )
            .unwrap()
    );
    assert_eq!(
        selector.decision(false, [320, 200]).analysis.status,
        "no_change"
    );
    let mut selector = Selection::new(thumbnail(None, [0; 3]), 1, 0);
    assert!(
        selector
            .push(
                thumbnail(Some([3, 3, 5, 5]), [80, 200, 128]),
                2,
                100,
                400,
                0,
                1000
            )
            .unwrap()
    );
    let decision = selector.decision(true, [320, 200]);
    assert_eq!(decision.sequence, 2);
    assert_eq!(decision.analysis.regions, vec![[30, 30, 50, 50]]);
}
#[test]
fn waits_for_window_content_instead_of_intermediate_plateau() {
    let mut selector = Selection::new(thumbnail(None, [0; 3]), 1, 0);
    assert!(
        !selector
            .push(
                thumbnail(Some([2, 2, 20, 15]), [100, 128, 128]),
                2,
                100,
                400,
                700,
                1000
            )
            .unwrap()
    );
    assert!(
        !selector
            .push(
                thumbnail(Some([2, 2, 20, 15]), [150, 128, 128]),
                3,
                710,
                800,
                700,
                1000
            )
            .unwrap()
    );
    assert!(
        selector
            .push(
                thumbnail(Some([2, 2, 20, 15]), [150, 128, 128]),
                4,
                800,
                1000,
                700,
                1000
            )
            .unwrap()
    );
    assert_eq!(selector.decision(true, [320, 200]).sequence, 3);
}
#[test]
fn moving_content_is_not_reported_as_settled_at_action_boundary() {
    let mut selector = Selection::new(thumbnail(None, [0; 3]), 1, 0);
    assert!(
        !selector
            .push(
                thumbnail(Some([2, 2, 20, 15]), [150, 128, 128]),
                2,
                100,
                180,
                0,
                1000
            )
            .unwrap()
    );
    assert!(
        !selector
            .push(
                thumbnail(Some([2, 2, 20, 15]), [200, 128, 128]),
                3,
                180,
                220,
                0,
                1000
            )
            .unwrap()
    );
    assert_eq!(
        selector.decision(false, [320, 200]).analysis.status,
        "changing"
    );
}
