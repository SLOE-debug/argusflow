use super::*;

#[test]
fn missing_pattern_is_distinct_from_provider_failure() {
    assert_eq!(
        pattern_failure("pattern", windows::core::Error::empty()).kind(),
        FailureKind::Unsupported
    );
    for (code, expected) in [
        (0x80004002u32, FailureKind::Unsupported),
        (0x80004003, FailureKind::Native),
    ] {
        let error = windows::core::Error::from_hresult(windows::core::HRESULT(code as i32));
        assert_eq!(pattern_failure("pattern", error).kind(), expected);
    }
}
