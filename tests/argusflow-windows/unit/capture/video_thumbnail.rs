use super::*;
#[test]
fn area_average_preserves_chroma_and_handles_coded_padding() {
    let bytes = [
        16, 235, 0, 0, 16, 235, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 90, 160, 0, 0, 90, 160, 0, 0,
    ];
    let result = thumbnail(&bytes, 2, 2, 4, 4).unwrap();
    assert_eq!((result.width, result.height), (1, 1));
    assert_eq!(result.pixels, vec![[125, 90, 160]]);
    assert!(thumbnail(&bytes[..16], 2, 2, 4, 4).is_err());
}
