//! 有界结构化观察与按捕获顺序脱敏，最后按事件时间发布唯一时间线。

use crate::{
    EventEvidence, EventTimeline, EvidenceCollector, RawInput, RecordingDiagnostic, RecordingTrace,
    ingestion::CapturedInput, redaction::InputRedactor,
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

/// 限制内存中的输入事实数量；原始序号与丢弃计数保留缺口。
const MAX_TRACE_EVENTS: usize = 100_000;
/// 防止慢速 provider 积累无界任务。
const MAX_PENDING: usize = 16;

#[cfg(test)]
pub(crate) async fn record(
    receiver: Receiver<CapturedInput>,
    collector: Arc<EvidenceCollector>,
    dropped: Arc<AtomicU64>,
    recording_id: uuid::Uuid,
    started_at_unix_ms: u64,
    processed: Arc<AtomicU64>,
) -> RecordingTrace {
    record_with_privacy(
        receiver,
        collector,
        dropped,
        recording_id,
        started_at_unix_ms,
        processed,
        crate::RecordingPrivacy::default(),
    )
    .await
}

/// 隐私策略仅属于本次会话，不修改共享 collector。
pub(crate) async fn record_with_privacy(
    mut receiver: Receiver<CapturedInput>,
    collector: Arc<EvidenceCollector>,
    dropped: Arc<AtomicU64>,
    recording_id: uuid::Uuid,
    started_at_unix_ms: u64,
    processed: Arc<AtomicU64>,
    privacy: crate::RecordingPrivacy,
) -> RecordingTrace {
    let mut pending = FuturesOrdered::new();
    let mut source_closed = false;
    let mut timeline = EventTimeline::default();
    let mut redactor = InputRedactor::new(privacy.input());
    let mut previous_sequence = 0;
    loop {
        tokio::select! {
            input = receiver.recv(), if !source_closed && pending.len() < MAX_PENDING => {
                match input { Some(input) => pending.push_back(collect_input(input, collector.clone())), None => source_closed = true }
            }
            Some((mut input, evidence)) = pending.next(), if !pending.is_empty() => {
                if input.event.sequence != previous_sequence + 1 {
                    redactor.reset();
                    input.diagnostics.push(RecordingDiagnostic::InputGap);
                }
                previous_sequence = input.event.sequence;
                if timeline.events.len() < MAX_TRACE_EVENTS {
                    let mut evidence = evidence;
                    if privacy.input() {
                        if let Some(target) = evidence.as_mut() {
                            if let Some(snapshot) = target.ui_snapshot.as_mut() {
                                if snapshot.entity.sensitivity != argusflow_core::FieldSensitivity::Normal {
                                    snapshot.entity.semantics.name = None;
                                    snapshot.entity.ancestors.iter_mut().for_each(|item| item.name = None);
                                    target.diagnostics.push(RecordingDiagnostic::Redacted);
                                }
                            }
                        }
                    }
                    let mut event = redactor.sanitize(input.event, input.elapsed_ms, input.decoded, evidence, input.diagnostics);
                    if let RawInput::Clipboard { content, .. } = &mut event.input {
                        if privacy.clipboard() {
                            *content = crate::ClipboardContent::Unavailable;
                            event.diagnostics.push(RecordingDiagnostic::Redacted);
                        } else { *content = input.clipboard.unwrap_or(crate::ClipboardContent::Unavailable); }
                    }
                    timeline.events.push(event);
                    processed.store(timeline.events.len() as u64, Ordering::Relaxed);
                } else { dropped.fetch_add(1, Ordering::Relaxed); }
            }
            else => break,
        }
        if source_closed && pending.is_empty() {
            break;
        }
    }
    timeline.compact_pointer_motion();
    RecordingTrace {
        screen: crate::ScreenTimeline::default(),
        schema_version: 3,
        recording_id,
        started_at_unix_ms,
        timeline,
        dropped_events: dropped.load(Ordering::Relaxed),
    }
}

/// 结构化观察保留输入时上下文；主要截图来自并行的操作后采样。
async fn collect_input(
    mut input: CapturedInput,
    collector: Arc<EvidenceCollector>,
) -> (CapturedInput, Option<EventEvidence>) {
    let mut evidence = EventEvidence::default();
    let late = input.captured_at.elapsed()
        + Duration::from_millis(input.captured_at_ms.saturating_sub(input.elapsed_ms))
        > Duration::from_millis(150)
        || input
            .diagnostics
            .contains(&RecordingDiagnostic::LateInspection);
    match input.context.take() {
        Some(Ok(context)) => {
            if let Some(probe) = input.probe.filter(|_| !late) {
                evidence = collector
                    .collect(
                        context,
                        probe,
                        input.captured_at_ms + input.captured_at.elapsed().as_millis() as u64,
                        input.elapsed_ms,
                    )
                    .await;
            } else {
                evidence.context = Some(context);
                if late {
                    evidence
                        .diagnostics
                        .push(RecordingDiagnostic::LateInspection);
                }
            }
        }
        Some(Err(reason)) => evidence
            .diagnostics
            .push(RecordingDiagnostic::ContextUnavailable { reason }),
        None => {}
    }
    if matches!(input.probe, Some(argusflow_core::InspectionProbe::Focus))
        && input.focus_epoch.load(Ordering::Relaxed) != input.expected_epoch
    {
        if let Some(snapshot) = evidence.ui_snapshot.take() {
            evidence
                .diagnostics
                .push(RecordingDiagnostic::InspectionFailed {
                    backend: snapshot.backend,
                    reason: argusflow_core::InspectionFailure::ContextChanged,
                });
        }
    }
    // 操作后采样和 PNG 编码与结构化查询并行，返回实际像素的持久化结果。
    if let Some(screenshot) = input.screenshot.take() {
        match screenshot
            .await
            .unwrap_or(Err(argusflow_core::InspectionFailure::Unavailable))
        {
            Ok(screenshot) => evidence.screenshot = Some(screenshot),
            Err(reason) => evidence
                .diagnostics
                .push(RecordingDiagnostic::ScreenshotUnavailable { reason }),
        }
    }
    if let Some(target) = input.click_target.take() {
        match target
            .await
            .unwrap_or(Err(argusflow_core::InspectionFailure::Unavailable))
        {
            Ok(target) => evidence.click_target = Some(target),
            Err(reason) => evidence
                .diagnostics
                .push(RecordingDiagnostic::ScreenshotUnavailable { reason }),
        }
    }
    let evidence = (evidence != EventEvidence::default()).then_some(evidence);
    (input, evidence)
}
