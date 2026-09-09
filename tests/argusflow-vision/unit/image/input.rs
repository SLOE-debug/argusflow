use super::*;
use argusflow_core::OperationOptions;
#[test]
fn transparent_and_bgra_pixels_have_explicit_meaning() {
    let image = ImageInput::Pixels {
        width: 2,
        height: 1,
        stride: 8,
        format: PixelFormat::Bgra,
        bytes: vec![0, 0, 0, 0, 0, 0, 255, 255],
    }
    .decode(
        &OcrConfig::new("unused"),
        &Operation::new(OperationOptions::default()),
    )
    .unwrap();
    assert_eq!(image.get_pixel(0, 0).0, [255, 255, 255]);
    assert_eq!(image.get_pixel(1, 0).0, [255, 0, 0]);
}
#[test]
fn corrupt_and_oversized_input_fail_without_inference() {
    let config = OcrConfig::new("unused");
    assert!(
        ImageInput::Encoded(vec![1, 2, 3])
            .decode(&config, &Operation::new(OperationOptions::default()))
            .is_err()
    );
    assert!(
        ImageInput::Pixels {
            width: u32::MAX,
            height: u32::MAX,
            stride: 0,
            format: PixelFormat::Rgb,
            bytes: vec![]
        }
        .validate(&config)
        .is_err()
    );
}
