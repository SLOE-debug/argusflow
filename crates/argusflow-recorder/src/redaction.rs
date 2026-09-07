//! 原始键码与布局文本只允许经过此出口进入可序列化 Trace。

use crate::{
    EventEvidence, InputPhase, PhysicalEvent, PhysicalInput, RawInput, RawTraceEvent, RecordedText,
    RecordingDiagnostic, input::DecodedKey,
};
use argusflow_core::{FieldSensitivity, KeyChord};

/// Up 必须继承对应 Down 的遮盖状态，防止通过释放键码反推密码。
pub(crate) struct InputRedactor {
    /// 默认 unknown/sensitive；必须成功检查 Down 后才允许暴露对应 Up。
    redacted_keys: [bool; 256],
    /// 自动重复 down 必须继承本次物理按压的遮盖状态。
    pressed_keys: [bool; 256],
    /// 给主键 up 关联已经确认的 PressKey，不依赖可逆键码。
    held_chords: [Option<KeyChord>; 256],
}

impl Default for InputRedactor {
    fn default() -> Self {
        Self {
            redacted_keys: [true; 256],
            pressed_keys: [false; 256],
            held_chords: std::array::from_fn(|_| None),
        }
    }
}

impl InputRedactor {
    /// 事件缺口后清空所有允许状态，不从后续释放事件泄露未知输入。
    pub(crate) fn reset(&mut self) {
        self.redacted_keys.fill(true);
        self.held_chords.fill(None);
        self.pressed_keys.fill(false);
    }

    pub(crate) fn sanitize(
        &mut self,
        event: PhysicalEvent,
        elapsed_ms: u64,
        decoded: DecodedKey,
        target: Option<EventEvidence>,
        mut diagnostics: Vec<RecordingDiagnostic>,
    ) -> RawTraceEvent {
        if let Some(reason) = decoded.failure {
            diagnostics.push(RecordingDiagnostic::KeyboardDecode { reason });
        }
        let input = match event.input {
            PhysicalInput::Window { window, change } => RawInput::Window { window, change },
            PhysicalInput::Clipboard { sequence_number } => RawInput::Clipboard {
                sequence_number,
                content: crate::ClipboardContent::Unavailable,
            },
            PhysicalInput::Mouse {
                point,
                button,
                phase,
            } => RawInput::Mouse {
                point,
                button,
                phase,
            },
            PhysicalInput::Move { point } => RawInput::Move { point },
            PhysicalInput::Wheel {
                point,
                delta,
                horizontal,
            } => RawInput::Wheel {
                point,
                delta,
                horizontal,
            },
            PhysicalInput::Key {
                virtual_key,
                scan_code,
                phase,
                flags,
            } => {
                let sensitive = target
                    .as_ref()
                    .and_then(|target| target.ui_snapshot.as_ref().map(|snapshot| &snapshot.entity))
                    .is_none_or(|entity| {
                        entity.sensitivity != FieldSensitivity::Normal || !entity.editable
                    });
                let redacted = if phase == InputPhase::Down {
                    let held_sensitive = self
                        .pressed_keys
                        .get(virtual_key as usize)
                        .copied()
                        .unwrap_or(false)
                        && self
                            .redacted_keys
                            .get(virtual_key as usize)
                            .copied()
                            .unwrap_or(true);
                    let redacted = sensitive || decoded.failure.is_some() || held_sensitive;
                    if let Some(state) = self.redacted_keys.get_mut(virtual_key as usize) {
                        *state = redacted;
                    }
                    if let Some(state) = self.pressed_keys.get_mut(virtual_key as usize) {
                        *state = true;
                    }
                    if let Some(chord) = self.held_chords.get_mut(virtual_key as usize) {
                        *chord = decoded.chord.clone();
                    }
                    redacted
                } else {
                    self.redacted_keys
                        .get(virtual_key as usize)
                        .copied()
                        .unwrap_or(true)
                };
                if redacted {
                    diagnostics.push(RecordingDiagnostic::Redacted);
                }
                let chord = if phase == InputPhase::Up {
                    if let Some(state) = self.pressed_keys.get_mut(virtual_key as usize) {
                        *state = false;
                    }
                    self.held_chords
                        .get_mut(virtual_key as usize)
                        .and_then(Option::take)
                } else {
                    decoded.chord
                };
                RawInput::Key {
                    virtual_key: (!redacted).then_some(virtual_key),
                    scan_code: (!redacted).then_some(scan_code),
                    flags: (!redacted).then_some(flags),
                    phase,
                    text: decoded.text.map(|text| {
                        if redacted {
                            RecordedText::Redacted
                        } else {
                            RecordedText::Plain(text)
                        }
                    }),
                    chord,
                }
            }
        };
        RawTraceEvent {
            sequence: event.sequence,
            timestamp_ms: event.timestamp_ms,
            elapsed_ms,
            input,
            evidence: target,
            diagnostics,
        }
    }
}
