use super::*;

#[test]
fn system_navigation_survives_self_filter_but_process_scope_remains_strict() {
    for vk in [91, 92] {
        for down in [true, false] {
            let key = InputKind::Key {
                vk,
                scan: 0,
                down,
                repeat: false,
                extended: true,
            };
            assert!(accepts(key, 7, 7, None));
            assert!(!accepts(key, 7, 7, Some(8)));
        }
    }
    let text = InputKind::Key {
        vk: 65,
        scan: 0,
        down: true,
        repeat: false,
        extended: false,
    };
    assert!(!accepts(text, 7, 7, None));
    assert!(!accepts(InputKind::Context, 7, 7, None));
    assert!(!accepts(InputKind::Move, 7, 7, None));
    assert!(accepts(text, 8, 7, None));
}
