use super::*;
use argusflow_core::OperationOptions;
use image::Rgb;

fn block(y: f32) -> TextBlock {
    TextBlock {
        text: format!("row-{y}"),
        confidence: 1.0,
        polygon: [
            ImagePoint { x: 10.0, y },
            ImagePoint { x: 50.0, y },
            ImagePoint {
                x: 50.0,
                y: y + 10.0,
            },
            ImagePoint {
                x: 10.0,
                y: y + 10.0,
            },
        ],
    }
}
#[test]
fn local_deletion_redetects_band_and_preserves_distant_text() {
    let mut cache = RecognitionCache::default();
    let before = RgbImage::from_pixel(400, 600, Rgb([255; 3]));
    cache.commit(
        before.clone(),
        OcrResult {
            blocks: vec![block(100.0), block(500.0)],
            width: 400,
            height: 600,
        },
    );
    let mut after = before;
    after.put_pixel(20, 105, Rgb([0; 3]));
    let mut heights = Vec::new();
    let result = process(
        after,
        &mut cache,
        &Operation::new(OperationOptions::default()),
        |image| {
            heights.push(image.height());
            Ok(vec![])
        },
        |_, _| panic!("no detected text"),
    )
    .unwrap();
    assert_eq!(heights.len(), 1);
    assert!(heights[0] < 200);
    assert_eq!(result.text(), "row-500");
}
#[test]
fn new_text_coordinates_are_projected_back_to_full_image() {
    let mut cache = RecognitionCache::default();
    let before = RgbImage::new(400, 600);
    cache.commit(
        before.clone(),
        OcrResult {
            blocks: vec![],
            width: 400,
            height: 600,
        },
    );
    let mut after = before;
    after.put_pixel(20, 300, Rgb([255; 3]));
    let result = process(
        after,
        &mut cache,
        &Operation::new(OperationOptions::default()),
        |image| {
            assert!(image.height() < 100);
            Ok(vec![block(30.0).polygon])
        },
        |_, polygon| {
            Ok(Some(TextBlock {
                text: "new".into(),
                confidence: 1.0,
                polygon,
            }))
        },
    )
    .unwrap();
    assert_eq!(result.blocks()[0].polygon()[0].y, 298.0);
}
#[test]
fn crop_edge_text_forces_complete_detection() {
    let mut cache = RecognitionCache::default();
    let before = RgbImage::new(400, 600);
    cache.commit(
        before.clone(),
        OcrResult {
            blocks: vec![block(500.0)],
            width: 400,
            height: 600,
        },
    );
    let mut after = before;
    after.put_pixel(20, 100, Rgb([255; 3]));
    let mut heights = Vec::new();
    let result = process(
        after,
        &mut cache,
        &Operation::new(OperationOptions::default()),
        |image| {
            heights.push(image.height());
            Ok(if image.height() == 600 {
                vec![]
            } else {
                vec![block(0.0).polygon]
            })
        },
        |_, _| panic!(),
    )
    .unwrap();
    assert_eq!(heights.len(), 2);
    assert_eq!(heights[1], 600);
    assert!(result.blocks().is_empty());
}
#[test]
fn resized_image_drops_out_of_bounds_cached_text() {
    let mut cache = RecognitionCache::default();
    cache.commit(
        RgbImage::new(400, 600),
        OcrResult {
            blocks: vec![block(500.0)],
            width: 400,
            height: 600,
        },
    );
    let result = process(
        RgbImage::new(400, 200),
        &mut cache,
        &Operation::new(OperationOptions::default()),
        |image| {
            assert_eq!(image.height(), 200);
            Ok(vec![])
        },
        |_, _| panic!(),
    )
    .unwrap();
    assert!(result.blocks().is_empty());
}
