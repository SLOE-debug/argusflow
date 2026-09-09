use super::*;
#[test]
fn negative_virtual_desktop_origin_is_normalized() {
    assert_eq!(normalize(-1920, -1920, 3840).unwrap(), 0);
    assert_eq!(normalize(1919, -1920, 3840).unwrap(), 65535);
    assert!(normalize(1920, -1920, 3840).is_err());
    assert!(normalize(0, 0, 1).is_err());
}
