//! 使用合成色块在真实 GPU 验证 shader 编译、缩放、旋转和不可变读回。
use super::{graphics, upload};
use crate::capture::gpu::{Scaler, Texture};
use argusflow_capture_contracts::*;
use std::time::{Duration, Instant};
#[test]
#[ignore = "requires hardware D3D11; synthetic textures only"]
fn full_frame_scale_and_rotation() {
    let graphics = graphics();
    let input = Texture::new(&graphics, 4, 2, false).unwrap();
    let bytes: Vec<u8> = (0..8).flat_map(|v| [v * 20, 0, 0, 255]).collect();
    upload(&graphics, &input, &bytes);
    for (rotation, size, expected) in [
        (
            Rotation::Identity,
            (4, 2),
            vec![0, 20, 40, 60, 80, 100, 120, 140],
        ),
        (
            Rotation::Clockwise90,
            (2, 4),
            vec![80, 0, 100, 20, 120, 40, 140, 60],
        ),
        (
            Rotation::Clockwise180,
            (4, 2),
            vec![140, 120, 100, 80, 60, 40, 20, 0],
        ),
        (
            Rotation::Clockwise270,
            (2, 4),
            vec![60, 140, 40, 120, 20, 100, 0, 80],
        ),
        (Rotation::Identity, (2, 1), vec![50, 90]),
    ] {
        let scaler = Scaler::new(&graphics, (4, 2), size, rotation).unwrap();
        scaler.submit(&graphics, &input.native).unwrap();
        let budget = ByteBudget::new(4096).unwrap();
        let start = Instant::now();
        let image = loop {
            if let Some(image) = scaler.poll(&graphics, &budget).unwrap() {
                break image;
            }
            assert!(start.elapsed() < Duration::from_secs(2));
            std::thread::sleep(Duration::from_millis(1));
        };
        let values: Vec<_> = image
            .bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[0])
            .collect();
        assert_eq!(values, expected, "{rotation:?}");
        assert_eq!((image.width(), image.height()), size);
    }
}
