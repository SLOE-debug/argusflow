use super::*;
#[test]
fn invalid_native_keys_are_rejected_before_injection() {
    assert!(native_key(Key::Letter('中')).is_err());
    assert!(native_key(Key::Function(0)).is_err());
    assert_eq!(native_key(Key::Function(12)).unwrap().0, VK_F12);
}
