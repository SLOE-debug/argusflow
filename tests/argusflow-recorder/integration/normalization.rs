use super::fixtures::*;
use argusflow_input_contracts::*;
use argusflow_recorder::*;

#[test]
fn win_release_remains_observable_after_the_foreground_changes() {
    let mut n = Normalizer::new(1000, 500, 4);
    assert!(n.push(1, key(91, true, 10)).is_none());
    let mut release = key(91, false, 20);
    release.window.epoch += 1;
    let action = n.push(2, release).unwrap();
    assert_eq!(action.kind, InteractionKind::SystemKey);
    assert_eq!(action.raw, [2]);
    assert!(requires_evidence(release));
    assert!(requires_evidence(key(65, false, 30))); // 松键驱动逐字符响应观察，不生成新文本操作。
    release.origin = InputOrigin::ArgusFlow;
    assert!(!requires_evidence(release));
}
#[test]
fn clicks_double_right_drag_and_scroll_keep_facts() {
    let mut n = Normalizer::new(1000, 500, 4);
    assert!(n.push(1, button(true, 10)).is_none());
    let first = n.push(2, button(false, 20)).unwrap();
    assert_eq!(first.kind, InteractionKind::Click);
    assert_eq!(first.raw, [1, 2]);
    n.push(3, button(true, 100));
    let second = n.push(4, button(false, 110)).unwrap();
    assert_eq!(second.kind, InteractionKind::DoubleClick);
    assert_eq!(second.related, Some(2));
    n.push(
        5,
        event(
            InputKind::Button {
                button: Button::Right,
                down: true,
            },
            200,
        ),
    );
    assert_eq!(
        n.push(
            6,
            event(
                InputKind::Button {
                    button: Button::Right,
                    down: false
                },
                210
            )
        )
        .unwrap()
        .kind,
        InteractionKind::Click
    );
    n.push(7, button(true, 300));
    let mut moved = event(InputKind::Move, 310);
    moved.point.x = 100;
    n.push(8, moved);
    assert_eq!(
        n.push(9, button(false, 320)).unwrap().kind,
        InteractionKind::Drag
    );
    for horizontal in [true, false] {
        let action = n
            .push(
                10,
                event(
                    InputKind::Wheel {
                        horizontal,
                        delta: -37,
                    },
                    400,
                ),
            )
            .unwrap();
        assert_eq!(action.kind, InteractionKind::Scroll);
        assert_eq!(action.raw, [10]);
    }
}
#[test]
fn pause_and_window_generation_do_not_complete_old_gestures() {
    let mut n = Normalizer::new(1000, 500, 4);
    n.push(1, button(true, 10));
    n.reset();
    assert_eq!(
        n.push(2, button(false, 20)).unwrap().kind,
        InteractionKind::Unresolved
    );
    n.push(3, button(true, 30));
    let mut release = button(false, 40);
    release.window.epoch += 1;
    assert_eq!(
        n.push(4, release).unwrap().kind,
        InteractionKind::Unresolved
    );
}
#[test]
fn ime_paste_delete_and_chords_never_claim_final_text() {
    let mut n = Normalizer::new(1000, 500, 4);
    n.push(1, key(17, true, 10));
    assert_eq!(
        n.push(2, key(86, true, 20)).unwrap().kind,
        InteractionKind::PasteUnconfirmed
    );
    n.reset();
    assert_eq!(
        n.push(3, key(229, true, 30)).unwrap().kind,
        InteractionKind::ImeUnconfirmed
    );
    assert_eq!(
        n.push(4, key(8, true, 40)).unwrap().kind,
        InteractionKind::DeleteUnconfirmed
    );
    n.reset();
    assert_eq!(
        n.push(5, key(65, true, 50)).unwrap().kind,
        InteractionKind::TextUnconfirmed
    );
    n.reset();
    n.push(6, key(17, true, 60));
    assert_eq!(
        n.push(7, key(83, true, 70)).unwrap().kind,
        InteractionKind::Chord
    );
}
#[test]
fn high_frequency_trajectory_is_bounded_without_mutating_raw_events() {
    let mut n = Normalizer::new(1000, 500, 4);
    n.push(1, button(true, 1));
    for id in 2..10000 {
        let mut e = event(InputKind::Move, id);
        e.point.x = id as i32;
        n.push(id as u64, e);
    }
    let action = n.push(10000, button(false, 10000)).unwrap();
    assert!(action.raw.len() <= 2048);
    assert_eq!(action.kind, InteractionKind::Drag);
}
