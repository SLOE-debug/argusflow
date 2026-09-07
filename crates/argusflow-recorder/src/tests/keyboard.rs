use super::*;

#[test]
fn decode_failures_distinguish_unsupported_chords_from_missing_context() {
    let mut decoder = KeyboardDecoder {
        state: [0; 256],
        dead_key: false,
    };
    assert_eq!(
        decoder.decode(0x100, 0, InputPhase::Down, None).failure,
        Some(KeyboardDecodeFailure::InvalidKey)
    );
    assert_eq!(
        decoder.decode(0x41, 30, InputPhase::Down, None).failure,
        Some(KeyboardDecodeFailure::MissingWindow)
    );
    decoder.decode(0xa2, 29, InputPhase::Down, None);
    assert_eq!(
        decoder.decode(0x70, 0, InputPhase::Down, None).failure,
        Some(KeyboardDecodeFailure::UnsupportedChord)
    );
}

#[test]
fn control_c_and_editing_keys_use_existing_workflow_chords() {
    let mut decoder = KeyboardDecoder {
        state: [0; 256],
        dead_key: false,
    };
    decoder.decode(0xa2, 29, InputPhase::Down, None);
    let copied = decoder.decode(0x43, 46, InputPhase::Down, None);
    assert_eq!(
        copied.chord,
        Some(KeyChord {
            key: KeyboardKey::Character { value: "C".into() },
            modifiers: vec![KeyboardModifier::Control]
        })
    );
    assert!(copied.text.is_none());
    decoder.decode(0xa2, 29, InputPhase::Up, None);
    for (code, expected) in [
        (0x08, KeyboardKey::Backspace),
        (0x2e, KeyboardKey::Delete),
        (0x09, KeyboardKey::Tab),
        (0x0d, KeyboardKey::Enter),
        (0x25, KeyboardKey::ArrowLeft),
    ] {
        assert_eq!(
            decoder.decode(code, 0, InputPhase::Down, None).chord,
            Some(KeyChord {
                key: expected,
                modifiers: vec![]
            })
        );
    }
}

#[test]
fn altgr_is_not_misclassified_as_control_alt_shortcut() {
    let mut decoder = KeyboardDecoder {
        state: [0; 256],
        dead_key: false,
    };
    decoder.decode(0xa2, 29, InputPhase::Down, None);
    decoder.decode(0xa5, 56, InputPhase::Down, None);
    assert!(
        decoder
            .decode(0x45, 18, InputPhase::Down, None)
            .chord
            .is_none()
    );
}
