use super::*;
use argusflow_capture_contracts::{ByteBudget, PixelFormat, PixelImage};
use argusflow_core::OperationOptions;
use std::{hint::black_box, time::Instant};

#[test]
#[ignore = "固定 1362×860 像素基准，显式运行并比较优化前后耗时"]
fn ocr_pixel_comparison_timing() {
    let (width, height) = (1362, 860);
    let size = width as usize * height as usize * 4;
    let budget = ByteBudget::new(size * 2).unwrap();
    let image = |bytes| {
        PixelImage::new(
            width,
            height,
            width as usize * 4,
            PixelFormat::Bgrx8,
            bytes,
            budget.reserve(size).unwrap(),
        )
        .unwrap()
    };
    let a = image(vec![255; size]);
    let b = image(vec![255; size]);
    let region = PixelRect::new(0, 0, width, height).unwrap();
    let operation = Operation::new(OperationOptions::default());
    for (name, left, right) in [("same_frame", &a, &a), ("equal_frames", &a, &b)] {
        let start = Instant::now();
        for _ in 0..5 {
            assert!(!black_box(
                has_changes(
                    ImageView::region(left, region).unwrap(),
                    ImageView::region(right, region).unwrap(),
                    1,
                    &operation
                )
                .unwrap()
            ));
        }
        println!(
            "{name}: average_ms={:.3}",
            start.elapsed().as_secs_f64() * 200.0
        );
    }
    let rgb = vec![255; width as usize * height as usize * 3];
    let mut changed = rgb.clone();
    changed[width as usize * 400 * 3 + 200] = 0;
    let start = Instant::now();
    let regions = changed_regions(
        ImageView::triples(width, height, &rgb).unwrap(),
        ImageView::triples(width, height, &changed).unwrap(),
        DifferencePolicy::default(),
        &operation,
    )
    .unwrap();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].pixels, 1);
    println!(
        "sparse_difference_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
}
