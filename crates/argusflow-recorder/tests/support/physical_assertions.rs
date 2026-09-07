//! 实体录制验收的独立断言，只检查固定测试输入及脱敏后的结构。

use argusflow_core::{FieldSensitivity, KeyboardKey};
use argusflow_recorder::{
    InputPhase, RawInput, RecordedOperation, RecordedText, RecordingTrace, ResolutionBackend,
};

/// Hook → ingestion → UIA → normalization → persistence 的最小完整验收。
pub fn assert_physical_trace(trace: &RecordingTrace, window_handle: u64) {
    print_input_diagnostics(trace);
    let records = &trace.normalized.records;
    for automation_id in ["1001", "1002"] {
        assert!(
            records.iter().any(|record| {
                matches!(record.operation, RecordedOperation::Click { .. })
                    && record.target.backend == ResolutionBackend::Uia
                    && record
                        .target
                        .context
                        .as_ref()
                        .is_some_and(|context| context.window.handle == window_handle)
                    && record.target.entity.as_ref().is_some_and(|entity| {
                        entity.semantics.automation_id.as_deref() == Some(automation_id)
                    })
                    && record.target.preferred_selector.is_some()
            }),
            "missing physical Click resolved by UIA for fixture field {automation_id}"
        );
    }
    // 允许输入间隔产生多个 TypeText，但必须完整且顺序正确地保留普通字段文字。
    let ordinary: String = records
        .iter()
        .filter_map(|record| {
            if record
                .target
                .entity
                .as_ref()?
                .semantics
                .automation_id
                .as_deref()?
                != "1001"
            {
                return None;
            }
            match &record.operation {
                RecordedOperation::TypeText {
                    text: RecordedText::Plain(text),
                } => Some(text.as_str()),
                _ => None,
            }
        })
        .collect();
    assert_eq!(
        ordinary, "argus",
        "ordinary fixture input was lost, altered or unnecessarily redacted"
    );
    assert!(
        records.iter().any(|record| {
            matches!(
                record.operation,
                RecordedOperation::TypeText {
                    text: RecordedText::Redacted
                }
            ) && record
                .target
                .entity
                .as_ref()
                .is_some_and(|entity| entity.sensitivity == FieldSensitivity::Sensitive)
        }),
        "missing redacted password semantic input"
    );
    assert!(records.iter().any(|record| {
        matches!(&record.operation, RecordedOperation::PressKey { chord } if chord.key == KeyboardKey::Enter)
    }), "missing Enter PressKey");
    let mut password_down = 0;
    for event in &trace.raw.events {
        if let RawInput::Key {
            text: Some(RecordedText::Redacted),
            virtual_key,
            scan_code,
            flags,
            phase,
            ..
        } = &event.input
        {
            assert!(
                virtual_key.is_none() && scan_code.is_none() && flags.is_none(),
                "redacted raw key retained reversible codes"
            );
            if *phase == InputPhase::Down {
                password_down += 1;
            }
        }
    }
    assert!(
        password_down >= 8,
        "not all eight password characters were captured and redacted"
    );
    assert!(
        !serde_json::to_string(trace).unwrap().contains("secret42"),
        "password plaintext leaked into recording trace"
    );
    assert_eq!(trace.dropped_events, 0, "recording dropped physical inputs");
    println!(
        "physical end-to-end: both UIA Clicks, argus TypeText, password redaction, Enter PressKey, no dropped input"
    );
}

/// 失败定位只打印类别、时序和 fixture ID，不打印文字、键码或密码。
fn print_input_diagnostics(trace: &RecordingTrace) {
    for event in &trace.raw.events {
        if let RawInput::Key {
            phase: InputPhase::Down,
            text,
            chord,
            ..
        } = &event.input
        {
            let category = if chord.is_some() {
                "chord"
            } else {
                match text {
                    Some(RecordedText::Plain(_)) => "plain",
                    Some(RecordedText::Redacted) => "redacted",
                    None => "no_text",
                }
            };
            let target = event.target.as_ref();
            let field = target
                .and_then(|target| target.entity.as_ref())
                .and_then(|entity| entity.semantics.automation_id.as_deref());
            println!(
                "input seq={} elapsed={}ms kind={category} field={field:?} backend={:?} input_diagnostics={:?} target_diagnostics={:?}",
                event.sequence,
                event.elapsed_ms,
                target.map(|target| target.backend),
                event.diagnostics,
                target.map(|target| &target.diagnostics)
            );
        }
    }
}
