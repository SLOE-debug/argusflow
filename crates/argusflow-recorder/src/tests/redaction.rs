use super::fixtures::target;
use crate::{input::DecodedKey, redaction::InputRedactor, *};
use argusflow_core::{FieldSensitivity, KeyChord, KeyboardKey, KeyboardModifier};

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
    let mut redactor = InputRedactor::default();
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
    let normalized = TraceNormalizer::normalize(&RawTrace {
        events: vec![down, up],
    });
    assert!(
        normalized.records.is_empty(),
        "IME key codes cannot become fabricated committed text"
    );
    assert_eq!(
        normalized.diagnostics,
        vec![RecordingDiagnostic::KeyboardDecode {
            reason: KeyboardDecodeFailure::InputMethodActive
        }]
    );
}

#[test]
fn password_and_unknown_redact_both_down_and_up_and_normalized_trace() {
    for sensitivity in [FieldSensitivity::Sensitive, FieldSensitivity::Unknown] {
        let mut redactor = InputRedactor::default();
        let mut target = target();
        target.entity.as_mut().unwrap().sensitivity = sensitivity;
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
        let raw = RawTrace {
            events: vec![down, up],
        };
        let normalized = TraceNormalizer::normalize(&raw);
        assert!(!serde_json::to_string(&raw).unwrap().contains("TOP_SECRET"));
        assert!(
            !serde_json::to_string(&normalized)
                .unwrap()
                .contains("TOP_SECRET")
        );
        assert_eq!(
            normalized.records[0].operation,
            RecordedOperation::TypeText {
                text: RecordedText::Redacted
            }
        );
    }
}

#[test]
fn normal_text_is_retained_but_gap_revokes_release_permission() {
    let mut redactor = InputRedactor::default();
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
    let mut redactor = InputRedactor::default();
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
