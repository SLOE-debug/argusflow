use super::fixtures::target;
use crate::{input::DecodedKey, redaction::InputRedactor, *};
use argusflow_core::{FieldSensitivity, KeyChord, KeyboardKey, KeyboardModifier};

#[test]
fn default_raw_recording_preserves_unknown_and_sensitive_down_and_up() {
    for target in [
        None,
        Some({
            let mut evidence = target();
            evidence.ui_snapshot.as_mut().unwrap().entity.sensitivity = FieldSensitivity::Sensitive;
            evidence
        }),
    ] {
        let mut redactor = InputRedactor::default();
        for phase in [InputPhase::Down, InputPhase::Up] {
            let recorded = redactor.sanitize(
                event(1, phase),
                1,
                DecodedKey {
                    text: Some("原始内容".into()),
                    ..Default::default()
                },
                target.clone(),
                vec![],
            );
            assert!(matches!(
                recorded.input,
                RawInput::Key {
                    virtual_key: Some(65),
                    text: Some(RecordedText::Plain(_)),
                    ..
                }
            ));
            assert!(
                !recorded
                    .diagnostics
                    .contains(&RecordingDiagnostic::Redacted)
            );
        }
    }
}

fn event(sequence: u64, phase: InputPhase) -> PhysicalEvent {
    PhysicalEvent {
        sequence,
        timestamp_ms: sequence as u32,
        input: PhysicalInput::Key {
            virtual_key: 65,
            scan_code: 30,
            flags: 0,
            phase,
        },
    }
}

#[test]
fn ime_failure_survives_in_ai_trace_without_leaking_down_or_up_codes() {
    let mut redactor = InputRedactor::new(true);
    let down = redactor.sanitize(
        event(1, InputPhase::Down),
        1,
        DecodedKey {
            failure: Some(KeyboardDecodeFailure::InputMethodActive),
            ..Default::default()
        },
        Some(target()),
        vec![],
    );
    let up = redactor.sanitize(
        event(2, InputPhase::Up),
        2,
        DecodedKey::default(),
        None,
        vec![],
    );
    for input in [&down.input, &up.input] {
        assert!(matches!(
            input,
            RawInput::Key {
                virtual_key: None,
                scan_code: None,
                flags: None,
                text: None,
                ..
            }
        ));
    }
    assert!(
        down.diagnostics
            .contains(&RecordingDiagnostic::KeyboardDecode {
                reason: KeyboardDecodeFailure::InputMethodActive
            })
    );
    assert_eq!(
        EventTimeline {
            events: vec![down, up]
        }
        .events
        .len(),
        2
    );
}

#[test]
fn password_and_unknown_redact_both_down_and_up_and_normalized_trace() {
    for sensitivity in [FieldSensitivity::Sensitive, FieldSensitivity::Unknown] {
        let mut redactor = InputRedactor::new(true);
        let mut target = target();
        target.ui_snapshot.as_mut().unwrap().entity.sensitivity = sensitivity;
        let down = redactor.sanitize(
            event(1, InputPhase::Down),
            1,
            DecodedKey {
                text: Some("TOP_SECRET".into()),
                ..Default::default()
            },
            Some(target),
            vec![],
        );
        let up = redactor.sanitize(
            event(2, InputPhase::Up),
            2,
            DecodedKey::default(),
            None,
            vec![],
        );
        for input in [&down.input, &up.input] {
            assert!(matches!(
                input,
                RawInput::Key {
                    virtual_key: None,
                    scan_code: None,
                    ..
                }
            ));
        }
        let raw = EventTimeline {
            events: vec![down, up],
        };
        assert!(!serde_json::to_string(&raw).unwrap().contains("TOP_SECRET"));
        assert_eq!(raw.events.len(), 2);
        assert!(matches!(
            raw.events[0].input,
            RawInput::Key {
                text: Some(RecordedText::Redacted),
                ..
            }
        ));
    }
}

#[test]
fn normal_text_is_retained_but_gap_revokes_release_permission() {
    let mut redactor = InputRedactor::new(true);
    let down = redactor.sanitize(
        event(1, InputPhase::Down),
        1,
        DecodedKey {
            text: Some("A".into()),
            ..Default::default()
        },
        Some(target()),
        vec![],
    );
    assert!(matches!(
        down.input,
        RawInput::Key {
            virtual_key: Some(65),
            text: Some(RecordedText::Plain(_)),
            ..
        }
    ));
    redactor.reset();
    let up = redactor.sanitize(
        event(3, InputPhase::Up),
        3,
        DecodedKey::default(),
        None,
        vec![],
    );
    assert!(matches!(
        up.input,
        RawInput::Key {
            virtual_key: None,
            scan_code: None,
            ..
        }
    ));
}

#[test]
fn shortcuts_are_semantic_and_auto_repeat_cannot_unredact_a_held_key() {
    let mut redactor = InputRedactor::new(true);
    redactor.sanitize(
        event(1, InputPhase::Down),
        1,
        DecodedKey::default(),
        None,
        vec![],
    );
    let repeated = redactor.sanitize(
        event(2, InputPhase::Down),
        2,
        DecodedKey {
            text: Some("A".into()),
            ..Default::default()
        },
        Some(target()),
        vec![],
    );
    assert!(matches!(
        repeated.input,
        RawInput::Key {
            text: Some(RecordedText::Redacted),
            ..
        }
    ));
    let chord = KeyChord {
        key: KeyboardKey::Character { value: "C".into() },
        modifiers: vec![KeyboardModifier::Control],
    };
    let down = redactor.sanitize(
        event(3, InputPhase::Down),
        3,
        DecodedKey {
            chord: Some(chord.clone()),
            ..Default::default()
        },
        None,
        vec![],
    );
    let up = redactor.sanitize(
        event(4, InputPhase::Up),
        4,
        DecodedKey::default(),
        None,
        vec![],
    );
    assert!(matches!(down.input, RawInput::Key { chord: Some(recorded), .. } if recorded == chord));
    assert!(matches!(up.input, RawInput::Key { chord: Some(recorded), .. } if recorded == chord));
}
