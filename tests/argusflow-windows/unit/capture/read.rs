use super::*;

#[test]
fn rotated_pixels_preserve_stride_colors_and_budget() {
    let budget = ByteBudget::new(1024).unwrap();
    for (rotation, width, height, order) in [
        (Rotation::Identity, 2, 3, vec![1, 2, 3, 4, 5, 6]),
        (Rotation::Clockwise90, 3, 2, vec![5, 3, 1, 6, 4, 2]),
        (Rotation::Clockwise180, 2, 3, vec![6, 5, 4, 3, 2, 1]),
        (Rotation::Clockwise270, 3, 2, vec![2, 4, 6, 1, 3, 5]),
    ] {
        let mut bytes = vec![99; 36];
        for y in 0..3 {
            for x in 0..2 {
                bytes[y * 12 + x * 4..y * 12 + x * 4 + 4].copy_from_slice(&[
                    (y * 2 + x + 1) as u8,
                    42,
                    7,
                    0,
                ]);
            }
        }
        let image = PixelImage::new(
            2,
            3,
            12,
            PixelFormat::Bgrx8,
            bytes,
            budget.reserve(36).unwrap(),
        )
        .unwrap();
        let result = rotate(image, rotation, &budget).unwrap();
        assert_eq!((result.width(), result.height()), (width, height));
        for (index, value) in order.into_iter().enumerate() {
            let y = index / width as usize;
            let x = index % width as usize;
            assert_eq!(
                &result.bytes()[y * result.stride() + x * 4..y * result.stride() + x * 4 + 4],
                &[value, 42, 7, 0]
            );
        }
        drop(result);
        assert_eq!(budget.used(), 0);
    }
}
