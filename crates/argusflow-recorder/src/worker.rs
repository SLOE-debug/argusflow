//! 有界并发语义检查与严格输入顺序的持久化前脱敏。

use crate::{
    RawTrace, RecordingDiagnostic, RecordingTrace, ResolutionBackend, ResolvedTarget,
    TargetResolver, TraceNormalizer, ingestion::CapturedInput, redaction::InputRedactor,
};
use futures_util::{StreamExt, stream::FuturesOrdered};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc::Receiver;

/// 有界 trace 长度，达到上限只计缺口，不继续增长内存。
const MAX_TRACE_EVENTS: usize = 100_000;
/// 并发 provider 请求上限，防止慢 UIA/OCR 导致无界 task 积累。
const MAX_PENDING: usize = 16;

/// 并发解析但按接收顺序返回；release/up 不能越过尚未脱敏的 down。
pub(crate) async fn record(
    mut receiver: Receiver<CapturedInput>,
    resolver: Arc<TargetResolver>,
    dropped: Arc<AtomicU64>,
    recording_id: uuid::Uuid,
    started_at_unix_ms: u64,
    processed: Arc<AtomicU64>,
) -> RecordingTrace {
    let mut pending = FuturesOrdered::new();
    let mut source_closed = false;
    let mut raw = RawTrace::default();
    let mut redactor = InputRedactor::default();
    let mut previous_sequence = 0;
    loop {
        tokio::select! {
            input = receiver.recv(), if !source_closed && pending.len() < MAX_PENDING => {
                match input {
                    Some(input) => pending.push_back(resolve_input(input, resolver.clone())),
                    None => source_closed = true,
                }
            }
            Some((mut input, target)) = pending.next(), if !pending.is_empty() => {
                if input.event.sequence != previous_sequence + 1 {
                    redactor.reset();
                    input.diagnostics.push(RecordingDiagnostic::InputGap);
                }
                previous_sequence = input.event.sequence;
                if raw.events.len() < MAX_TRACE_EVENTS {
                    raw.events.push(redactor.sanitize(input.event, input.elapsed_ms, input.decoded,
                        target, input.diagnostics));
                    processed.store(raw.events.len() as u64, Ordering::Relaxed);
                } else { dropped.fetch_add(1, Ordering::Relaxed); }
            }
            else => break,
        }
        if source_closed && pending.is_empty() {
            break;
        }
    }
    let mut normalized = TraceNormalizer::normalize(&raw);
    let dropped_events = dropped.load(Ordering::Relaxed);
    if dropped_events > 0 {
        normalized.diagnostics.push(RecordingDiagnostic::InputGap);
    }
    RecordingTrace {
        schema_version: 1,
        recording_id,
        started_at_unix_ms,
        raw,
        normalized,
        dropped_events,
    }
}

/// 检查开始过晚时保存 coordinate 事实，不把当前 UI 当成历史 UI。
async fn resolve_input(
    mut input: CapturedInput,
    resolver: Arc<TargetResolver>,
) -> (CapturedInput, Option<ResolvedTarget>) {
    let Some(probe) = input.probe else {
        return (input, None);
    };
    let late = input.captured_at.elapsed() > Duration::from_millis(150)
        || input
            .diagnostics
            .contains(&RecordingDiagnostic::LateInspection);
    let context = input.context.take();
    let mut target = match context {
        Some(Ok(context)) if !late => resolver.resolve(context, probe).await,
        Some(Ok(context)) => TargetResolver::fallback(
            Some(context),
            probe,
            vec![RecordingDiagnostic::LateInspection],
        ),
        Some(Err(reason)) => TargetResolver::fallback(
            None,
            probe,
            vec![RecordingDiagnostic::Fallback {
                backend: ResolutionBackend::Coordinate,
                reason,
            }],
        ),
        None => TargetResolver::fallback(None, probe, vec![RecordingDiagnostic::LateInspection]),
    };
    if matches!(probe, argusflow_core::InspectionProbe::Focus)
        && input.focus_epoch.load(Ordering::Relaxed) != input.expected_epoch
    {
        target = TargetResolver::fallback(
            target.context,
            probe,
            vec![RecordingDiagnostic::Fallback {
                backend: target.backend,
                reason: argusflow_core::InspectionFailure::ContextChanged,
            }],
        );
    }
    (input, Some(target))
}
