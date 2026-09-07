//! 只消费已脱敏 Raw Trace 的确定性 normalization；不访问 UI 或猜测文本内容。

use crate::{
    InputPhase, MouseButton, NormalizedSemanticTrace, RawInput, RawTrace, RawTraceEvent,
    RecordedOperation, RecordedText, RecordingDiagnostic, ResolvedTarget, SemanticRecord,
};
use argusflow_core::ScreenPoint;

/// 默认归一化器；设置只影响分组，不改变 raw 事实。
pub struct TraceNormalizer;

/// 鼠标 down 的候选语义，move/up 验证后才生成 Click。
struct PendingClick<'a> {
    /// 起始原始事件。
    down: &'a RawTraceEvent,
    /// 按下点，用于识别拖拽。
    point: ScreenPoint,
    /// 必须由同一鼠标按键释放。
    button: MouseButton,
    /// 任意超出点击容差的移动均不可逆地标记为拖拽。
    dragged: bool,
}

impl TraceNormalizer {
    /// 按原始顺序规范化，输入缺口与不支持操作都切断文本合并。
    pub fn normalize(raw: &RawTrace) -> NormalizedSemanticTrace {
        let mut trace = NormalizedSemanticTrace::default();
        let mut click: Option<PendingClick<'_>> = None;
        let mut text: Option<SemanticRecord> = None;
        let mut previous_sequence = 0;
        for event in &raw.events {
            if event.sequence != previous_sequence + 1
                || event.diagnostics.contains(&RecordingDiagnostic::InputGap)
            {
                flush_text(&mut text, &mut trace);
                click = None;
                trace.diagnostics.push(RecordingDiagnostic::InputGap);
            }
            previous_sequence = event.sequence;
            for diagnostic in &event.diagnostics {
                if matches!(
                    diagnostic,
                    RecordingDiagnostic::UnsupportedInput
                        | RecordingDiagnostic::KeyboardDecode { .. }
                ) {
                    flush_text(&mut text, &mut trace);
                    // 未提交字符可能没有可回放步骤，诊断仍必须出现在独立语义层中。
                    if !trace.diagnostics.contains(diagnostic) {
                        trace.diagnostics.push(diagnostic.clone());
                    }
                }
            }
            match &event.input {
                RawInput::Mouse {
                    point,
                    button,
                    phase: InputPhase::Down,
                } => {
                    flush_text(&mut text, &mut trace);
                    if click.take().is_some() {
                        trace.diagnostics.push(RecordingDiagnostic::UnpairedMouse);
                    }
                    click = Some(PendingClick {
                        down: event,
                        point: *point,
                        button: *button,
                        dragged: false,
                    });
                }
                RawInput::Move { point } => {
                    if let Some(click) = &mut click {
                        click.dragged |= moved(click.point, *point, click.down.target.as_ref());
                    }
                }
                RawInput::Mouse {
                    point,
                    button,
                    phase: InputPhase::Up,
                } => {
                    let Some(pending) = click.take() else {
                        trace.diagnostics.push(RecordingDiagnostic::UnpairedMouse);
                        continue;
                    };
                    if pending.button != *button {
                        trace.diagnostics.push(RecordingDiagnostic::UnpairedMouse);
                        continue;
                    }
                    if pending.dragged || moved(pending.point, *point, pending.down.target.as_ref())
                    {
                        trace
                            .diagnostics
                            .push(RecordingDiagnostic::UnsupportedInput);
                        continue;
                    }
                    if let Some(target) = pending.down.target.clone() {
                        let mut record = record(
                            pending.down,
                            RecordedOperation::Click { button: *button },
                            target,
                        );
                        record.raw_event_ids.push(event.sequence);
                        record.original_input.push(event.input.clone());
                        record.ended_ms = event.elapsed_ms;
                        push_record(record, &mut trace);
                    }
                }
                RawInput::Wheel { .. } => {
                    flush_text(&mut text, &mut trace);
                    if let Some(click) = &mut click {
                        click.dragged = true;
                    }
                    trace
                        .diagnostics
                        .push(RecordingDiagnostic::UnsupportedInput);
                }
                RawInput::Key {
                    phase: InputPhase::Down,
                    chord: Some(chord),
                    ..
                } => {
                    if let Some(click) = &mut click {
                        click.dragged = true;
                    }
                    flush_text(&mut text, &mut trace);
                    if let Some(target) = event.target.clone() {
                        push_record(
                            record(
                                event,
                                RecordedOperation::PressKey {
                                    chord: chord.clone(),
                                },
                                target,
                            ),
                            &mut trace,
                        );
                    }
                }
                RawInput::Key {
                    phase: InputPhase::Down,
                    text: Some(value),
                    ..
                } => {
                    if let Some(click) = &mut click {
                        click.dragged = true;
                    }
                    let Some(target) = event.target.as_ref() else {
                        continue;
                    };
                    let can_merge = text.as_ref().is_some_and(|record| {
                        event.elapsed_ms.saturating_sub(record.ended_ms) <= 1000
                            && same_target(&record.target, target)
                            && matches!(
                                (&record.operation, value),
                                (
                                    RecordedOperation::TypeText {
                                        text: RecordedText::Plain(_)
                                    },
                                    RecordedText::Plain(_)
                                ) | (
                                    RecordedOperation::TypeText {
                                        text: RecordedText::Redacted
                                    },
                                    RecordedText::Redacted
                                )
                            )
                    });
                    if !can_merge {
                        flush_text(&mut text, &mut trace);
                        text = Some(record(
                            event,
                            RecordedOperation::TypeText {
                                text: value.clone(),
                            },
                            target.clone(),
                        ));
                    } else if let Some(record) = &mut text {
                        if let (
                            RecordedOperation::TypeText {
                                text: RecordedText::Plain(buffer),
                            },
                            RecordedText::Plain(value),
                        ) = (&mut record.operation, value)
                        {
                            buffer.push_str(value);
                        }
                        record.raw_event_ids.push(event.sequence);
                        record.original_input.push(event.input.clone());
                        record.ended_ms = event.elapsed_ms;
                    }
                }
                RawInput::Key {
                    phase: InputPhase::Up,
                    chord: Some(chord),
                    ..
                } => {
                    if let Some(record) = trace.records.last_mut()
                        && matches!(&record.operation, RecordedOperation::PressKey { chord: recorded } if recorded == chord)
                    {
                        record.raw_event_ids.push(event.sequence);
                        record.original_input.push(event.input.clone());
                        record.ended_ms = event.elapsed_ms;
                    }
                }
                RawInput::Key {
                    phase: InputPhase::Up,
                    ..
                } => {
                    // Up 不重复字符，但保留它与当前输入组的追溯关系。
                    if let Some(record) = &mut text {
                        record.raw_event_ids.push(event.sequence);
                        record.original_input.push(event.input.clone());
                        record.ended_ms = event.elapsed_ms;
                    }
                }
                RawInput::Key { .. } => {}
            }
        }
        flush_text(&mut text, &mut trace);
        if click.is_some() {
            trace.diagnostics.push(RecordingDiagnostic::UnpairedMouse);
        }
        trace
    }
}

fn same_target(left: &ResolvedTarget, right: &ResolvedTarget) -> bool {
    left.backend == right.backend
        && left.context.as_ref().map(|context| context.window)
            == right.context.as_ref().map(|context| context.window)
        && match (&left.entity, &right.entity) {
            (Some(left), Some(right)) => {
                left.identity == right.identity && left.sensitivity == right.sensitivity
            }
            // Unknown focus 不跨事件合并：同一窗口可能包含多个无法观察的输入框。
            _ => false,
        }
}

fn moved(start: ScreenPoint, end: ScreenPoint, target: Option<&ResolvedTarget>) -> bool {
    let tolerance = target
        .and_then(|target| target.context.as_ref())
        .map_or(5.0, |context| 5.0 * f64::from(context.dpi.max(96)) / 96.0);
    (f64::from(start.x) - f64::from(end.x)).abs() > tolerance
        || (f64::from(start.y) - f64::from(end.y)).abs() > tolerance
}

fn record(
    event: &RawTraceEvent,
    operation: RecordedOperation,
    target: ResolvedTarget,
) -> SemanticRecord {
    SemanticRecord {
        sequence: 0,
        started_ms: event.elapsed_ms,
        ended_ms: event.elapsed_ms,
        raw_event_ids: vec![event.sequence],
        original_input: vec![event.input.clone()],
        operation,
        target,
    }
}

fn push_record(mut record: SemanticRecord, trace: &mut NormalizedSemanticTrace) {
    record.sequence = trace.records.len() as u64 + 1;
    trace.records.push(record);
}

fn flush_text(text: &mut Option<SemanticRecord>, trace: &mut NormalizedSemanticTrace) {
    if let Some(record) = text.take() {
        push_record(record, trace);
    }
}
