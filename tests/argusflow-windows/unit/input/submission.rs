use super::*;
fn key(code: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(code),
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                ..Default::default()
            },
        },
    }
}
#[test]
fn partial_injection_releases_only_held_keys_and_never_replays_down() {
    let events = [key(17, false), key(65, false), key(65, true), key(17, true)];
    let mut calls = 0;
    let error = submit_with(&events, |input| {
        calls += 1;
        if calls == 1 {
            (
                3,
                windows::core::Error::from_hresult(windows::core::HRESULT(0x80070005u32 as i32)),
            )
        } else {
            assert_eq!(input.len(), 1);
            unsafe {
                assert_eq!(input[0].Anonymous.ki.wVk.0, 17);
                assert!(input[0].Anonymous.ki.dwFlags.contains(KEYEVENTF_KEYUP));
            }
            (1, windows::core::Error::empty())
        }
    })
    .unwrap_err();
    assert_eq!(calls, 2);
    assert_eq!(error.kind(), FailureKind::Native);
}
#[test]
fn zero_injection_does_not_release_unpressed_keys() {
    let mut calls = 0;
    assert!(
        submit_with(&[key(17, false)], |_| {
            calls += 1;
            (0, windows::core::Error::empty())
        })
        .is_err()
    );
    assert_eq!(calls, 1);
}
