use super::*;
use argusflow_core::{Failure, FailureKind, OperationOptions};
use image::Rgb;
use std::cell::Cell;
fn polygon(x: f32) -> [ImagePoint; 4] {
    [
        ImagePoint { x, y: 5.0 },
        ImagePoint {
            x: x + 10.0,
            y: 5.0,
        },
        ImagePoint {
            x: x + 10.0,
            y: 15.0,
        },
        ImagePoint { x, y: 15.0 },
    ]
}
fn run(
    image: RgbImage,
    cache: &mut RecognitionCache,
    polygons: Vec<[ImagePoint; 4]>,
    calls: &Cell<usize>,
) -> OcrResult {
    process(
        image,
        cache,
        &Operation::new(OperationOptions::default()),
        |_| Ok(polygons),
        |_, polygon| {
            calls.set(calls.get() + 1);
            Ok(Some(TextBlock {
                text: format!("{}", calls.get()),
                confidence: 1.0,
                polygon,
            }))
        },
    )
    .unwrap()
}
#[test]
fn changes_only_recognize_affected_complete_text_blocks() {
    let mut cache = RecognitionCache::default();
    let calls = Cell::new(0);
    let mut image = RgbImage::from_pixel(80, 30, Rgb([255; 3]));
    let polygons = vec![polygon(5.0), polygon(50.0)];
    let first = run(image.clone(), &mut cache, polygons.clone(), &calls);
    assert_eq!(calls.get(), 2);
    image.put_pixel(8, 8, Rgb([254, 255, 255]));
    let second = run(image, &mut cache, polygons, &calls);
    assert_eq!(calls.get(), 3);
    assert_eq!(second.blocks()[0].polygon(), first.blocks()[0].polygon());
    assert_eq!(second.blocks()[1].text(), first.blocks()[1].text());
}
#[test]
fn unchanged_image_skips_both_models() {
    let mut cache = RecognitionCache::default();
    let calls = Cell::new(0);
    let image = RgbImage::new(80, 30);
    run(image.clone(), &mut cache, vec![polygon(5.0)], &calls);
    let result = process(
        image,
        &mut cache,
        &Operation::new(OperationOptions::default()),
        |_| panic!("detector called"),
        |_, _| panic!("recognizer called"),
    )
    .unwrap();
    assert_eq!(result.blocks().len(), 1);
}
#[test]
fn removed_and_new_text_follow_current_detector() {
    let mut cache = RecognitionCache::default();
    let calls = Cell::new(0);
    let mut image = RgbImage::new(80, 30);
    run(image.clone(), &mut cache, vec![polygon(5.0)], &calls);
    image.put_pixel(0, 0, Rgb([1; 3]));
    let result = run(image, &mut cache, vec![polygon(50.0)], &calls);
    assert_eq!(result.blocks().len(), 1);
    assert_eq!(result.blocks()[0].polygon(), &polygon(50.0));
    assert_eq!(calls.get(), 2);
}
#[test]
fn failure_and_cancellation_do_not_replace_successful_cache() {
    let mut cache = RecognitionCache::default();
    let calls = Cell::new(0);
    let image = RgbImage::new(80, 30);
    run(image.clone(), &mut cache, vec![polygon(5.0)], &calls);
    let mut changed = image.clone();
    changed.put_pixel(8, 8, Rgb([1; 3]));
    let error = process(
        changed,
        &mut cache,
        &Operation::new(OperationOptions::default()),
        |_| Ok(vec![polygon(5.0)]),
        |_, _| Err(Failure::new(FailureKind::Native, "test", "failure").into()),
    );
    assert!(error.is_err());
    let operation = Operation::new(OperationOptions::default());
    operation.cancel();
    assert!(
        process(
            image.clone(),
            &mut cache,
            &operation,
            |_| panic!(),
            |_, _| panic!()
        )
        .is_err()
    );
    run(image, &mut cache, vec![], &calls);
    assert_eq!(calls.get(), 1);
}
