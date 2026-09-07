//! 事件摄入：及时冻结窗口、剪贴板、像素与键盘布局，再投递只读结构化检查。

use crate::{
    EvidenceCollector, InputPhase, PhysicalEvent, PhysicalInput, RecordingDiagnostic,
    input::DecodedKey, keyboard::KeyboardDecoder,
};
use argusflow_core::{InspectionContext, InspectionFailure, InspectionProbe};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::Receiver,
    },
    time::Instant,
};
use tokio::sync::mpsc::Sender;
use windows::Win32::System::SystemInformation::GetTickCount;

/// 只存在于内存的待解析事件；不实现序列化或 Debug。
pub(crate) struct CapturedInput {
    /// 已冻结帧的异步保存结果，不在 UIA/CDP 完成后重新截图。
    pub screenshot: Option<crate::screenshot_pipeline::PendingScreenshot>,
    /// 通知对应版本的剪贴板副本，不在异步查询中重新读取。
    pub clipboard: Option<crate::ClipboardContent>,
    /// Hook 最小字段。
    pub event: PhysicalEvent,
    /// 相对录制开始的事件时间，支持 u32 Win32 时钟回绕。
    pub elapsed_ms: u64,
    /// 及时冻结的窗口上下文；禁止在慢速队列尾端重新猜窗口。
    pub context: Option<Result<InspectionContext, InspectionFailure>>,
    /// 鼠标点、键盘焦点或事件窗口根；高频移动不反查元素。
    pub probe: Option<InspectionProbe>,
    /// Worker 目标键盘布局转换结果。
    pub decoded: DecodedKey,
    /// Worker 排队的单调时钟起点。
    pub captured_at: Instant,
    /// captured_at 对应录制相对毫秒；包含事件至摄入的延迟。
    pub captured_at_ms: u64,
    /// 输入缺口、延迟等事实。
    pub diagnostics: Vec<RecordingDiagnostic>,
    /// 焦点可能改变的输入代数，防止把稍后的普通字段套到先前敏感键入上。
    pub focus_epoch: Arc<AtomicU64>,
    /// 当前事件开始检查前看到的代数。
    pub expected_epoch: u64,
}

/// 从最小事件实时读取廉价 Win32 元数据；这里仍不执行 UIA/CDP/OCR。
pub(crate) fn ingest(
    receiver: Receiver<PhysicalEvent>,
    sender: Sender<CapturedInput>,
    resolver: Arc<EvidenceCollector>,
    dropped: Arc<AtomicU64>,
    started_tick: u32,
    screenshots: crate::screenshot_pipeline::ScreenshotWriter,
) {
    let mut keyboard = KeyboardDecoder::new();
    let mut previous_sequence = 0;
    let mut clock = crate::event_clock::EventClock::new(started_tick);
    let focus_epoch = Arc::new(AtomicU64::new(0));
    while let Ok(event) = receiver.recv() {
        let mut diagnostics = Vec::new();
        if event.sequence != previous_sequence + 1 {
            keyboard = KeyboardDecoder::new();
            focus_epoch.fetch_add(1, Ordering::Relaxed);
            diagnostics.push(RecordingDiagnostic::InputGap);
        }
        previous_sequence = event.sequence;
        let elapsed_ms = clock.advance(event.timestamp_ms);
        let probe = match event.input {
            PhysicalInput::Mouse { point, .. } | PhysicalInput::Wheel { point, .. } => {
                Some(InspectionProbe::Point(point))
            }
            PhysicalInput::Key { .. } => Some(InspectionProbe::Focus),
            PhysicalInput::Clipboard { .. } => Some(InspectionProbe::Focus),
            PhysicalInput::Window { .. } => Some(InspectionProbe::Window),
            PhysicalInput::Move { .. } => None,
        };
        // SAFETY: 只读取系统事件时钟；wrapping_sub 保留约 49 天回绕语义。
        let late = unsafe { GetTickCount() }.wrapping_sub(event.timestamp_ms) > 150;
        if late {
            diagnostics.push(RecordingDiagnostic::LateInspection);
        }
        let context = if late {
            None
        } else {
            match event.input {
                PhysicalInput::Window { window, .. } => Some(resolver.window_context(window)),
                PhysicalInput::Key { .. } => Some(resolver.context(InspectionProbe::Focus)),
                _ => probe.map(|probe| resolver.context(probe)),
            }
        };
        let clipboard = match event.input {
            PhysicalInput::Clipboard { sequence_number } => {
                Some(crate::clipboard::capture(sequence_number))
            }
            _ => None,
        };
        // 为每个非移动事件尽可能保存证据；先冻结图像，再开始任何元素查询。
        let capture_started = Instant::now();
        let capture_delay = unsafe { GetTickCount() }.wrapping_sub(event.timestamp_ms);
        let screenshot = context
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .and_then(|context| {
                let point = match event.input {
                    PhysicalInput::Mouse { point, .. } | PhysicalInput::Wheel { point, .. } => {
                        Some(point)
                    }
                    _ => None,
                };
                if capture_delay > 150 {
                    diagnostics.push(RecordingDiagnostic::ScreenshotUnavailable {
                        reason: InspectionFailure::Timeout,
                    });
                    return None;
                }
                let frame = resolver.capture(context);
                let duration_ms = capture_started.elapsed().as_millis() as u64;
                match frame.and_then(|frame| {
                    screenshots.submit(
                        event.sequence,
                        frame,
                        elapsed_ms + u64::from(capture_delay),
                        duration_ms,
                        point,
                        matches!(
                            event.input,
                            PhysicalInput::Mouse {
                                phase: InputPhase::Down,
                                ..
                            }
                        ),
                    )
                }) {
                    Ok(evidence) => Some(evidence),
                    Err(reason) => {
                        diagnostics.push(RecordingDiagnostic::ScreenshotUnavailable { reason });
                        None
                    }
                }
            });
        let decoded = match event.input {
            PhysicalInput::Key {
                virtual_key,
                scan_code,
                phase,
                ..
            } => keyboard.decode(
                virtual_key,
                scan_code,
                phase,
                context.as_ref().and_then(|result| result.as_ref().ok()),
            ),
            _ => DecodedKey::default(),
        };
        if matches!(
            event.input,
            PhysicalInput::Mouse {
                phase: InputPhase::Down,
                ..
            }
        ) || matches!(event.input, PhysicalInput::Window { .. })
            || decoded.chord.is_some()
        {
            focus_epoch.fetch_add(1, Ordering::Relaxed);
        }
        let expected_epoch = focus_epoch.load(Ordering::Relaxed);
        let input = CapturedInput {
            event,
            screenshot,
            clipboard,
            elapsed_ms,
            context,
            probe,
            decoded,
            captured_at: capture_started,
            captured_at_ms: elapsed_ms + u64::from(capture_delay),
            diagnostics,
            focus_epoch: focus_epoch.clone(),
            expected_epoch,
        };
        // 满队列不可阻塞及时窗口探测；真实 sequence 缺口由时间线明确保留。
        if sender.try_send(input).is_err() {
            dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}
