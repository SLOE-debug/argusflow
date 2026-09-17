use super::*;
#[test]
fn limited_range_black_white_and_primary_red() {
    let pixels = rgba(&[16, 235, 16, 235, 128, 128], 2, 2, 2, 2).unwrap();
    assert_eq!(&pixels[..8], &[0, 0, 0, 255, 255, 255, 255, 255]);
    let red = rgba(&[63, 63, 63, 63, 102, 240], 2, 2, 2, 2).unwrap();
    assert_eq!(&red[..4], &[255, 1, 0, 255]);
}
#[test]
fn chroma_uses_coded_height_and_stride_not_display_height() {
    let padded = [
        16, 235, 0, 0, 16, 235, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 128, 0, 0, 128, 128, 0, 0,
    ];
    assert_eq!(
        rgba(&padded, 2, 2, 4, 4).unwrap(),
        rgba(&[16, 235, 16, 235, 128, 128], 2, 2, 2, 2).unwrap()
    );
    assert!(rgba(&padded[..16], 2, 2, 4, 4).is_err());
    assert!(rgba(&padded, 2, 2, 4, -4).is_err());
}
