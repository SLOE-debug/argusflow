//! 显式原生验收，常规测试不下载模型、不要求安装 ORT。
use argusflow_core::{FailureKind, OperationOptions};
use argusflow_vision::{Device, ImageInput, ModelTier, OcrConfig, OcrEngine, PixelFormat};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use imageproc::geometric_transformations::{Interpolation, rotate_about_center};
use std::{io::Cursor, path::PathBuf};

const BILINGUAL: &[u8] = include_bytes!("../fixtures/bilingual.png");
const EXPECTED: &str = "ArgusFlow OCR 123\n中文识别测试 456";

fn config(tier: ModelTier, device: Device) -> OcrConfig {
    let deps = PathBuf::from(
        std::env::var_os("ARGUSFLOW_TEST_DEPS")
            .expect("set ARGUSFLOW_TEST_DEPS to the prepared .deps directory"),
    );
    let mut config = OcrConfig::new(&deps);
    config.tier = tier;
    config.device = device;
    if let Some(runtime) = std::env::var_os("ARGUSFLOW_TEST_RUNTIME") {
        config.runtime_directory = Some(runtime.into());
    }
    config
}
fn png(image: DynamicImage) -> ImageInput {
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png).unwrap();
    ImageInput::Encoded(bytes.into_inner())
}
fn validate(result: &argusflow_vision::OcrResult) {
    for block in result.blocks() {
        assert!((0.0..=1.0).contains(&block.confidence()));
        for p in block.polygon() {
            assert!(p.x.is_finite() && p.y.is_finite());
            assert!(p.x >= 0.0 && p.x < result.width() as f32);
            assert!(p.y >= 0.0 && p.y < result.height() as f32);
        }
    }
}
async fn exercise(tier: ModelTier, device: Device) -> argusflow_vision::OcrResult {
    let engine = OcrEngine::load(config(tier, device)).await.unwrap();
    let result = engine
        .recognize(ImageInput::Encoded(BILINGUAL.to_vec()))
        .await
        .unwrap();
    validate(&result);
    assert_eq!(result.text(), EXPECTED, "{tier:?}/{device:?}");
    assert_eq!(result.blocks().len(), 2);
    assert!(result.blocks()[0].polygon()[0].y < result.blocks()[1].polygon()[0].y);
    // 固定 PNG：文字、数字、中英文、多行、路径和编码字节入口。
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/argusflow-vision/fixtures/bilingual.png");
    assert_eq!(
        engine
            .recognize(ImageInput::Path(path))
            .await
            .unwrap()
            .text(),
        EXPECTED
    );
    let blank = png(DynamicImage::ImageRgb8(RgbImage::from_pixel(
        320,
        128,
        Rgb([255, 255, 255]),
    )));
    assert!(engine.recognize(blank).await.unwrap().blocks().is_empty());
    let transparent = png(DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        320,
        128,
        Rgba([0, 0, 0, 0]),
    )));
    assert!(
        engine
            .recognize(transparent)
            .await
            .unwrap()
            .blocks()
            .is_empty()
    );
    let original = image::load_from_memory(BILINGUAL).unwrap().to_rgba8();
    // 把背景变透明，保留字体反锯齿黑色覆盖率。
    let alpha = RgbaImage::from_fn(original.width(), original.height(), |x, y| {
        Rgba([0, 0, 0, 255 - original.get_pixel(x, y)[0]])
    });
    let input = ImageInput::Pixels {
        width: alpha.width(),
        height: alpha.height(),
        stride: alpha.width() as usize * 4,
        format: PixelFormat::Rgba,
        bytes: alpha.into_raw(),
    };
    assert_eq!(engine.recognize(input).await.unwrap().text(), EXPECTED);
    let original = image::load_from_memory(BILINGUAL).unwrap().to_rgb8();
    let mut canvas = RgbImage::from_pixel(1100, 500, Rgb([255, 255, 255]));
    image::imageops::replace(&mut canvas, &original, 100, 120);
    let rotated = rotate_about_center(
        &canvas,
        8.0_f32.to_radians(),
        Interpolation::Bilinear,
        Rgb([255, 255, 255]),
    );
    let rotated = engine
        .recognize(png(DynamicImage::ImageRgb8(rotated)))
        .await
        .unwrap();
    validate(&rotated);
    assert_eq!(rotated.text(), EXPECTED);
    assert!(
        rotated
            .blocks()
            .iter()
            .any(|block| (block.polygon()[0].y - block.polygon()[1].y).abs() > 10.0)
    );
    assert!(
        engine
            .recognize(ImageInput::Encoded(vec![1, 2, 3]))
            .await
            .is_err()
    );
    let oversized = ImageInput::Pixels {
        width: 20_000,
        height: 20_000,
        stride: 60_000,
        format: PixelFormat::Rgb,
        bytes: vec![],
    };
    assert_eq!(
        engine.recognize(oversized).await.unwrap_err().kind(),
        FailureKind::ResourceLimit
    );
    engine.shutdown(OperationOptions::default()).await.unwrap();
    result
}

#[tokio::test]
#[ignore = "requires explicitly prepared official models and ONNX Runtime; see docs/backend.md"]
async fn official_small_and_medium_cpu() {
    for tier in [ModelTier::Small, ModelTier::Medium] {
        exercise(tier, Device::Cpu).await;
    }
}
#[tokio::test]
#[ignore = "requires prepared CUDA runtime and NVIDIA GPU; set ARGUSFLOW_TEST_RUNTIME to runtime/cuda"]
async fn official_cpu_cuda_parity() {
    for tier in [ModelTier::Small, ModelTier::Medium] {
        let cpu = exercise(tier, Device::Cpu).await;
        let cuda = exercise(tier, Device::Cuda { device_id: 0 }).await;
        assert_eq!(cpu.text(), cuda.text());
        for (a, b) in cpu.blocks().iter().zip(cuda.blocks()) {
            assert!((a.confidence() - b.confidence()).abs() < 0.04);
            for (a, b) in a.polygon().iter().zip(b.polygon()) {
                assert!((a.x - b.x).abs() < 8.0 && (a.y - b.y).abs() < 8.0);
            }
        }
    }
}
