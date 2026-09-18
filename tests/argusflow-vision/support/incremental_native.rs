//! 官方 PaddleOCR 模型验收局部新增、消失和不变帧；与全图首次识别比较。
use super::*;

#[tokio::test]
#[ignore = "需要本机官方 small 模型和 CPU ONNX Runtime"]
async fn paddle_incremental_added_removed_and_unchanged() {
    let engine = OcrEngine::load(config(ModelTier::Small, Device::Cpu))
        .await
        .unwrap();
    let mut recorded = Vec::new();
    let text = image::load_from_memory(BILINGUAL).unwrap().to_rgb8();
    let width = text.width() + 80;
    let height = text.height() * 4 + 400;
    let blank = RgbImage::from_pixel(width, height, Rgb([255; 3]));
    let mut first = blank.clone();
    image::imageops::replace(&mut first, &text, 40, 40);
    let mut opened = first.clone();
    image::imageops::replace(&mut opened, &text, 40, i64::from(text.height() + 200));
    let mut moved = blank.clone();
    image::imageops::replace(&mut moved, &text, 40, i64::from(text.height() * 2 + 200));
    for (name, frame, count) in [
        ("initial", first.clone(), 1),
        ("opened", opened.clone(), 2),
        ("unchanged", opened, 2),
        ("closed", first, 1),
        ("moved", moved, 1),
        ("removed", blank, 0),
    ] {
        let start = std::time::Instant::now();
        let actual = engine
            .recognize(png(DynamicImage::ImageRgb8(frame.clone())))
            .await
            .unwrap();
        println!(
            "{name}: elapsed={:?}, text={:?}",
            start.elapsed(),
            actual.text()
        );
        validate(&actual);
        assert_eq!(actual.blocks().len(), count * 2, "{name}");
        recorded.push((name, frame, actual.text()));
    }
    for (name, frame, text) in recorded {
        // 同一模型实例显式切换尺寸，清除增量基线后获取完整检测结果。
        engine
            .recognize(png(DynamicImage::ImageRgb8(RgbImage::new(32, 32))))
            .await
            .unwrap();
        let expected = engine
            .recognize(png(DynamicImage::ImageRgb8(frame)))
            .await
            .unwrap();
        assert_eq!(text, expected.text(), "{name}");
    }
    engine.shutdown(OperationOptions::default()).await.unwrap();
}
