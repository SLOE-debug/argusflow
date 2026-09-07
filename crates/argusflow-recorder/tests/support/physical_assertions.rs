//! 实体演示验收：原始事件与证据关联，不依赖 Click/TypeText 合成。

use argusflow_core::KeyboardKey;
use argusflow_recorder::{EvidenceBackend, InputPhase, RawInput, RecordedText, RecordingTrace};

/// 对固定原生 fixture 验证完整字符、鼠标事实、图像与密码键码遮盖。
pub fn assert_physical_trace(trace: &RecordingTrace, window_handle: u64) {
    let events = &trace.timeline.events;
    for automation_id in ["1001", "1002"] {
        assert!(
            events.iter().any(|event| {
                let Some(evidence) = &event.evidence else {
                    return false;
                };
                matches!(
                    event.input,
                    RawInput::Mouse {
                        phase: InputPhase::Down,
                        ..
                    }
                ) && evidence
                    .context
                    .as_ref()
                    .is_some_and(|context| context.window.handle == window_handle)
                    && evidence.ui_snapshot.as_ref().is_some_and(|snapshot| {
                        snapshot.backend == EvidenceBackend::Uia
                            && snapshot.entity.semantics.automation_id.as_deref()
                                == Some(automation_id)
                    })
                    && evidence.screenshot.is_some()
            }),
            "missing mouse down with UIA and image evidence for {automation_id}"
        );
    }
    let ordinary: String = events
        .iter()
        .filter_map(|event| {
            let snapshot = event.evidence.as_ref()?.ui_snapshot.as_ref()?;
            if snapshot.entity.semantics.automation_id.as_deref() != Some("1001") {
                return None;
            }
            match &event.input {
                RawInput::Key {
                    text: Some(RecordedText::Plain(text)),
                    phase: InputPhase::Down,
                    ..
                } => Some(text.as_str()),
                _ => None,
            }
        })
        .collect();
    assert_eq!(ordinary, "argus");
    assert!(events.iter().any(|event| matches!(&event.input, RawInput::Key { chord: Some(chord), .. } if chord.key == KeyboardKey::Enter)));
    let mut password_down = 0;
    for event in events {
        if let RawInput::Key {
            text: Some(RecordedText::Redacted),
            virtual_key,
            scan_code,
            flags,
            phase,
            ..
        } = &event.input
        {
            assert!(virtual_key.is_none() && scan_code.is_none() && flags.is_none());
            if *phase == InputPhase::Down {
                password_down += 1;
            }
        }
    }
    assert!(password_down >= 8);
    assert!(!serde_json::to_string(trace).unwrap().contains("secret42"));
    assert_eq!(trace.dropped_events, 0);
}
