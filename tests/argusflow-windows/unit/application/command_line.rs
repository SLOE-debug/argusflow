use super::command_line;
use std::ffi::OsStr;
#[test]
fn quotes_empty_spaces_backslashes_and_quotes_without_shell() {
    let encoded = command_line(
        OsStr::new(r"C:\Program Files\app.exe"),
        &[
            String::new(),
            "a b".into(),
            "tail\\".into(),
            "say\"hi".into(),
        ],
    )
    .unwrap();
    assert_eq!(
        String::from_utf16(&encoded[..encoded.len() - 1]).unwrap(),
        "\"C:\\Program Files\\app.exe\" \"\" \"a b\" \"tail\\\\\" \"say\\\"hi\""
    );
}
#[test]
fn rejects_embedded_nul() {
    assert!(command_line(OsStr::new("app.exe"), &["a\0b".into()]).is_err());
}
