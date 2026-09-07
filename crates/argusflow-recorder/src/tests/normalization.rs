use super::fixtures::*;
use crate::*;
use argusflow_core::{KeyChord, KeyboardKey, ScreenPoint};

#[test]
fn click_uses_mouse_down_target_and_preserves_original_inputs() {
    let down = raw(1, mouse(InputPhase::Down, 20));
    let mut up = raw(2, mouse(InputPhase::Up, 21));
    up.target = None;
    let trace = TraceNormalizer::normalize(&RawTrace {
        events: vec![down.clone(), up],
    });
    assert_eq!(trace.records.len(), 1);
    assert_eq!(
        trace.records[0].operation,
        RecordedOperation::Click {
            button: MouseButton::Left
        }
    );
    assert_eq!(trace.records[0].target, down.target.unwrap());
    assert_eq!(trace.records[0].raw_event_ids, [1, 2]);
    assert_eq!(trace.records[0].original_input.len(), 2);
}

#[test]
fn drag_out_and_back_is_never_a_click() {
    let trace = TraceNormalizer::normalize(&RawTrace {
        events: vec![
            raw(1, mouse(InputPhase::Down, 20)),
            raw(
                2,
                RawInput::Move {
                    point: ScreenPoint { x: 200, y: 20 },
                },
            ),
            raw(3, mouse(InputPhase::Up, 20)),
        ],
    });
    assert!(trace.records.is_empty());
    assert!(
        trace
            .diagnostics
            .contains(&RecordingDiagnostic::UnsupportedInput)
    );
}

#[test]
fn key_up_does_not_duplicate_text_and_tab_splits_input() {
    let chord = KeyChord {
        key: KeyboardKey::Tab,
        modifiers: vec![],
    };
    let key_up = RawInput::Key {
        virtual_key: Some(65),
        flags: Some(0),
        scan_code: Some(30),
        phase: InputPhase::Up,
        text: None,
        chord: None,
    };
    let trace = TraceNormalizer::normalize(&RawTrace {
        events: vec![
            raw(1, text("你")),
            raw(2, key_up),
            raw(3, text("好")),
            raw(
                4,
                RawInput::Key {
                    virtual_key: Some(9),
                    flags: Some(0),
                    scan_code: Some(15),
                    phase: InputPhase::Down,
                    text: None,
                    chord: Some(chord.clone()),
                },
            ),
            raw(5, text("B")),
        ],
    });
    assert_eq!(trace.records.len(), 3);
    assert_eq!(
        trace.records[0].operation,
        RecordedOperation::TypeText {
            text: RecordedText::Plain("你好".into())
        }
    );
    assert_eq!(trace.records[0].raw_event_ids, [1, 2, 3]);
    assert_eq!(
        trace.records[1].operation,
        RecordedOperation::PressKey { chord }
    );
}

#[test]
fn time_focus_and_sequence_gaps_prevent_text_merging() {
    for mode in 0..3 {
        let first = raw(1, text("A"));
        let mut second = raw(2, text("B"));
        match mode {
            0 => second.elapsed_ms = 2000,
            1 => {
                second
                    .target
                    .as_mut()
                    .unwrap()
                    .entity
                    .as_mut()
                    .unwrap()
                    .identity = "field-2".into()
            }
            _ => second.sequence = 3,
        }
        assert_eq!(
            TraceNormalizer::normalize(&RawTrace {
                events: vec![first, second]
            })
            .records
            .len(),
            2
        );
    }
}

#[test]
fn unpaired_mouse_and_wheel_are_diagnostics_not_actions() {
    let trace = TraceNormalizer::normalize(&RawTrace {
        events: vec![
            raw(1, mouse(InputPhase::Up, 20)),
            raw(
                2,
                RawInput::Wheel {
                    point: ScreenPoint { x: 20, y: 20 },
                    delta: 120,
                    horizontal: false,
                },
            ),
            raw(3, mouse(InputPhase::Down, 20)),
        ],
    });
    assert!(trace.records.is_empty());
    assert!(
        trace
            .diagnostics
            .contains(&RecordingDiagnostic::UnpairedMouse)
    );
    assert!(
        trace
            .diagnostics
            .contains(&RecordingDiagnostic::UnsupportedInput)
    );
}
