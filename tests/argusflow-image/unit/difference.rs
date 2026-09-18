use super::*;
use argusflow_core::OperationOptions;

#[test]
fn chunk_boundaries_and_last_pixel_preserve_thresholds() {
    let a = vec![[0; 3]; 1026];
    let mut b = a.clone();
    for i in [255, 256, 512, 1025] {
        b[i][2] = 2;
    }
    let regions = compare(&a, &b, 513, DifferencePolicy::default());
    assert_eq!(regions.iter().map(|r| r.pixels).sum::<usize>(), 4);
    assert!(
        compare(
            &a,
            &b,
            513,
            DifferencePolicy {
                threshold: 3,
                ..Default::default()
            }
        )
        .is_empty()
    );
}

#[test]
fn different_offsets_strides_and_alpha_compare_visible_colors() {
    use argusflow_capture_contracts::{ByteBudget, PixelFormat, PixelImage};
    let budget = ByteBudget::new(128).unwrap();
    let make = |format, stride, bytes: Vec<u8>| {
        let reservation = budget.reserve(bytes.len()).unwrap();
        PixelImage::new(2, 2, stride, format, bytes, reservation).unwrap()
    };
    let a = make(PixelFormat::Rgba8, 8, vec![0; 16]);
    let b = make(PixelFormat::Bgra8, 12, vec![255; 24]);
    let operation = Operation::new(OperationOptions::default());
    assert!(
        !has_changes(
            ImageView::region(&a, PixelRect::new(1, 0, 1, 2).unwrap()).unwrap(),
            ImageView::region(&b, PixelRect::new(0, 0, 1, 2).unwrap()).unwrap(),
            1,
            &operation,
        )
        .unwrap()
    );
    let mut bytes = vec![0; 16];
    bytes[8..12].copy_from_slice(&[255, 255, 255, 255]);
    let c = make(PixelFormat::Rgba8, 8, bytes);
    assert!(
        !has_changes(
            ImageView::region(&a, PixelRect::new(0, 0, 2, 2).unwrap()).unwrap(),
            ImageView::region(&c, PixelRect::new(0, 0, 2, 2).unwrap()).unwrap(),
            1,
            &operation,
        )
        .unwrap()
    );
}

fn compare(
    a: &[[u8; 3]],
    b: &[[u8; 3]],
    width: u32,
    policy: DifferencePolicy,
) -> Vec<ChangeRegion> {
    changed_regions(
        ImageView::triples(width, a.len() as u32 / width, a.as_flattened()).unwrap(),
        ImageView::triples(width, b.len() as u32 / width, b.as_flattened()).unwrap(),
        policy,
        &Operation::new(OperationOptions::default()),
    )
    .unwrap()
}
#[test]
fn exact_detects_single_pixel_and_thin_lines() {
    let a = vec![[0; 3]; 64];
    let mut b = a.clone();
    b[0][0] = 1;
    for row in 2..8 {
        b[row * 8 + 4][1] = 1;
    }
    let regions = compare(&a, &b, 8, DifferencePolicy::default());
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].bounds, PixelRect::new(4, 2, 1, 6).unwrap());
    assert_eq!(regions[1].pixels, 1);
}
#[test]
fn recording_filter_is_explicit_and_keeps_color_changes() {
    let a = vec![[80, 128, 128]; 64];
    let mut b = a.clone();
    for i in 0..8 {
        b[i * 8][0] = 255;
    }
    for y in 2..5 {
        for x in 3..6 {
            b[y * 8 + x][1] = 200;
        }
    }
    let regions = compare(
        &a,
        &b,
        8,
        DifferencePolicy {
            threshold: 12,
            min_pixels: 6,
            min_width: 2,
            min_height: 2,
        },
    );
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].bounds, PixelRect::new(3, 2, 3, 3).unwrap());
}
#[test]
fn four_connected_does_not_join_diagonals() {
    let a = [[0; 3]; 4];
    let b = [[1; 3], [0; 3], [0; 3], [1; 3]];
    assert_eq!(compare(&a, &b, 2, DifferencePolicy::default()).len(), 2);
}
#[test]
fn validates_layout_dimensions_and_threshold() {
    assert!(ImageView::triples(0, 2, &[]).is_err());
    assert!(ImageView::triples(2, 2, &[0; 3]).is_err());
    let a = ImageView::triples(1, 1, &[0; 3]).unwrap();
    let b = ImageView::triples(2, 1, &[0; 6]).unwrap();
    let operation = Operation::new(OperationOptions::default());
    assert!(has_changes(a, b, 1, &operation).is_err());
    assert!(has_changes(a, a, 0, &operation).is_err());
}
#[test]
fn cancelled_comparison_returns_error() {
    let a = ImageView::triples(1, 1, &[0; 3]).unwrap();
    let operation = Operation::new(OperationOptions::default());
    operation.cancel();
    assert!(has_changes(a, a, 1, &operation).is_err());
    assert!(changed_regions(a, a, DifferencePolicy::default(), &operation).is_err());
}
#[test]
fn bgrx_padding_and_row_padding_do_not_change_content() {
    use argusflow_capture_contracts::{ByteBudget, PixelFormat, PixelImage};
    let budget = ByteBudget::new(32).unwrap();
    let image = |bytes| {
        PixelImage::new(
            1,
            1,
            8,
            PixelFormat::Bgrx8,
            bytes,
            budget.reserve(8).unwrap(),
        )
        .unwrap()
    };
    let a = image(vec![3, 2, 1, 0, 9, 9, 9, 9]);
    let b = image(vec![3, 2, 1, 255, 0, 0, 0, 0]);
    let region = PixelRect::new(0, 0, 1, 1).unwrap();
    assert!(
        !has_changes(
            ImageView::region(&a, region).unwrap(),
            ImageView::region(&b, region).unwrap(),
            1,
            &Operation::new(OperationOptions::default())
        )
        .unwrap()
    );
}
