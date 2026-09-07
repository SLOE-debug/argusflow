//! Hook 后第一阶段 worker：及时冻结窗口与键盘布局，再投递异步语义检查。

use crate::{
    InputPhase, PhysicalEvent, PhysicalInput, RecordingDiagnostic, TargetResolver,
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
    /// Hook 最小字段。
    pub event: PhysicalEvent,
    /// 相对录制开始的事件时间，支持 u32 Win32 时钟回绕。
    pub elapsed_ms: u64,
    /// 及时冻结的窗口上下文；禁止在慢速队列尾端重新猜窗口。
    pub context: Option<Result<InspectionContext, InspectionFailure>>,
    /// Point/Focus 只为 mouse down / key down 创建。
    pub probe: Option<InspectionProbe>,
    /// Worker 目标键盘布局转换结果。
    pub decoded: DecodedKey,
    /// Worker 排队的单调时钟起点。
    pub captured_at: Instant,
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
    resolver: Arc<TargetResolver>,
    dropped: Arc<AtomicU64>,
    started_tick: u32,
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
            PhysicalInput::Mouse {
                point,
                phase: InputPhase::Down,
                ..
            } => Some(InspectionProbe::Point(point)),
            PhysicalInput::Key {
                phase: InputPhase::Down,
                virtual_key,
                ..
            } if !matches!(virtual_key, 0x10..=0x12 | 0xa0..=0xa5 | 0x5b | 0x5c) => {
                Some(InspectionProbe::Focus)
            }
            PhysicalInput::Mouse { .. }
            | PhysicalInput::Key { .. }
            | PhysicalInput::Move { .. }
            | PhysicalInput::Wheel { .. } => None,
        };
        // SAFETY: 只读取系统事件时钟；wrapping_sub 保留约 49 天回绕语义。
        let late = unsafe { GetTickCount() }.wrapping_sub(event.timestamp_ms) > 150;
        if late {
            diagnostics.push(RecordingDiagnostic::LateInspection);
        }
        let context = probe.filter(|_| !late).map(|probe| resolver.context(probe));
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
        ) || decoded.chord.is_some()
        {
            focus_epoch.fetch_add(1, Ordering::Relaxed);
        }
        let expected_epoch = focus_epoch.load(Ordering::Relaxed);
        let input = CapturedInput {
            event,
            elapsed_ms,
            context,
            probe,
            decoded,
            captured_at: Instant::now(),
            diagnostics,
            focus_epoch: focus_epoch.clone(),
            expected_epoch,
        };
        // 满队列不可阻塞及时窗口探测；异步 worker 根据真实 sequence 缺口切断 normalization。
        if sender.try_send(input).is_err() {
            dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}
