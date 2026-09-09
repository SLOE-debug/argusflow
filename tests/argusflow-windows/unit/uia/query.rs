use super::*;

#[test]
fn navigation_accepts_empty_success_but_propagates_native_failures() {
    assert!(
        optional_navigation(Err(windows::core::Error::empty()))
            .unwrap()
            .is_none()
    );
    for code in [0x80004003u32, 0x80004005] {
        let error = windows::core::Error::from_hresult(windows::core::HRESULT(code as i32));
        let failure = optional_navigation(Err(error)).unwrap_err();
        assert_eq!(failure.kind(), FailureKind::Native);
        assert_eq!(failure.stage(), "tree_navigation");
    }
}
