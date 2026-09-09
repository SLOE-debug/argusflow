//! 图像预算租约与边界验证。
use argusflow_capture_contracts::*;
#[test]
fn delivered_image_keeps_budget_until_last_clone() {
    let budget = ByteBudget::new(16).unwrap();
    let reservation = budget.reserve(16).unwrap();
    let image = PixelImage::new(2, 2, 8, PixelFormat::Bgrx8, vec![0; 16], reservation).unwrap();
    let clone = image.clone();
    drop(image);
    assert_eq!(budget.used(), 16);
    assert!(budget.reserve(1).is_err());
    drop(clone);
    assert_eq!(budget.used(), 0);
    assert_eq!(budget.peak(), 16);
}
#[test]
fn invalid_images_return_reservations() {
    let budget = ByteBudget::new(16).unwrap();
    assert!(
        PixelImage::new(
            2,
            2,
            7,
            PixelFormat::Bgrx8,
            vec![0; 16],
            budget.reserve(16).unwrap()
        )
        .is_err()
    );
    assert_eq!(budget.used(), 0);
    assert!(PixelRect::new(u32::MAX, 0, 1, 1).is_err());
    assert!(ScreenRect::new(i32::MAX, 0, 1, 1).is_err());
}
