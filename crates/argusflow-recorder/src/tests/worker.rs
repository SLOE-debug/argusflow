use super::fixtures::{context, entity};
use crate::{ingestion::CapturedInput, input::DecodedKey, *};
use argusflow_core::*;
use async_trait::async_trait;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

struct Window;
impl WindowInspector for Window {
    fn window_context(&self, _: WindowIdentity) -> Result<InspectionContext, InspectionFailure> {
        Ok(context())
    }
    fn context(&self, _: InspectionProbe) -> Result<InspectionContext, InspectionFailure> {
        Ok(context())
    }
}

struct Inspector(Arc<AtomicUsize>);
#[async_trait]
impl TargetInspector for Inspector {
    async fn inspect(
        &self,
        _: &InspectionContext,
        probe: InspectionProbe,
    ) -> Result<InspectedEntity, InspectionFailure> {
        self.0.fetch_add(1, Ordering::Relaxed);
        let delay = if matches!(probe, InspectionProbe::Point(ScreenPoint { x: 1, .. })) {
            30
        } else {
            1
        };
        tokio::time::sleep(Duration::from_millis(delay)).await;
        let mut entity = entity();
        entity.identity = format!("{probe:?}");
        Ok(entity)
    }
}

fn resolver(calls: Arc<AtomicUsize>) -> Arc<EvidenceCollector> {
    let inspector = Arc::new(Inspector(calls));
    Arc::new(EvidenceCollector::new(
        Arc::new(Window),
        inspector.clone(),
        inspector.clone(),
        Arc::new(super::fixtures::NoCapture),
    ))
}

fn captured(sequence: u64, phase: InputPhase, x: i32) -> CapturedInput {
    let point = ScreenPoint { x, y: 20 };
    let probe = (phase == InputPhase::Down).then_some(InspectionProbe::Point(point));
    CapturedInput {
        event: PhysicalEvent {
            sequence,
            timestamp_ms: sequence as u32,
            input: PhysicalInput::Mouse {
                point,
                button: MouseButton::Left,
                phase,
            },
        },
        screenshot: None,
        clipboard: None,
        captured_at_ms: sequence,
        elapsed_ms: sequence,
        context: probe.map(|_| Ok(context())),
        probe,
        decoded: DecodedKey::default(),
        captured_at: Instant::now(),
        diagnostics: vec![],
        focus_epoch: Arc::new(AtomicU64::new(0)),
        expected_epoch: 0,
    }
}

#[tokio::test]
async fn worker_compacts_movement_before_returning_the_recording() {
    let (sender, receiver) = tokio::sync::mpsc::channel(256);
    for sequence in 1..=200 {
        let mut input = captured(sequence, InputPhase::Up, sequence as i32);
        input.event.input = PhysicalInput::Move {
            point: ScreenPoint {
                x: sequence as i32,
                y: 0,
            },
        };
        sender.send(input).await.ok().unwrap();
    }
    drop(sender);
    let calls = Arc::new(AtomicUsize::new(0));
    let processed = Arc::new(AtomicU64::new(0));
    let trace = crate::worker::record(
        receiver,
        resolver(calls.clone()),
        Arc::new(AtomicU64::new(0)),
        uuid::Uuid::new_v4(),
        0,
        processed.clone(),
    )
    .await;
    assert_eq!(trace.timeline.events.len(), 1);
    assert!(
        matches!(&trace.timeline.events[0].input, RawInput::PointerMotion(motion) if motion.sample_count == 200 && motion.points.len() == 2)
    );
    assert_eq!(processed.load(Ordering::Relaxed), 200);
    assert_eq!(
        trace.dropped_events, 0,
        "intentional aggregation is not event loss"
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn concurrent_inspection_preserves_down_up_and_record_order() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = tokio::sync::mpsc::channel(8);
    for input in [
        captured(1, InputPhase::Down, 1),
        captured(2, InputPhase::Up, 1),
        captured(3, InputPhase::Down, 2),
        captured(4, InputPhase::Up, 2),
    ] {
        sender.send(input).await.ok().unwrap();
    }
    drop(sender);
    let trace = crate::worker::record(
        receiver,
        resolver(calls.clone()),
        Arc::new(AtomicU64::new(0)),
        uuid::Uuid::new_v4(),
        0,
        Arc::new(AtomicU64::new(0)),
    )
    .await;
    assert_eq!(
        trace
            .timeline
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert_eq!(trace.timeline.events.len(), 4);
    assert!(
        trace.timeline.events[0]
            .evidence
            .as_ref()
            .unwrap()
            .ui_snapshot
            .as_ref()
            .unwrap()
            .entity
            .identity
            .contains("x: 1")
    );
    assert!(
        trace.timeline.events[2]
            .evidence
            .as_ref()
            .unwrap()
            .ui_snapshot
            .as_ref()
            .unwrap()
            .entity
            .identity
            .contains("x: 2")
    );
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn late_events_skip_provider_and_focus_change_redacts_pending_keys() {
    for late in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let (sender, receiver) = tokio::sync::mpsc::channel(2);
        let mut input = captured(1, InputPhase::Down, 1);
        input.event.input = PhysicalInput::Key {
            virtual_key: 65,
            scan_code: 30,
            flags: 0,
            phase: InputPhase::Down,
        };
        input.probe = Some(InspectionProbe::Focus);
        input.decoded.text = Some("SECRET".into());
        if late {
            input.captured_at -= Duration::from_secs(1);
        } else {
            input.focus_epoch.store(1, Ordering::Relaxed);
        }
        sender.send(input).await.ok().unwrap();
        drop(sender);
        let trace = crate::worker::record(
            receiver,
            resolver(calls.clone()),
            Arc::new(AtomicU64::new(0)),
            uuid::Uuid::new_v4(),
            0,
            Arc::new(AtomicU64::new(0)),
        )
        .await;
        assert!(!serde_json::to_string(&trace).unwrap().contains("SECRET"));
        assert!(
            trace.timeline.events[0]
                .evidence
                .as_ref()
                .unwrap()
                .ui_snapshot
                .is_none()
        );
        if late {
            assert_eq!(calls.load(Ordering::Relaxed), 0);
        }
    }
}

#[tokio::test]
async fn cross_application_timeline_preserves_clipboard_window_wheel_and_frozen_pixels() {
    let (sender, receiver) = tokio::sync::mpsc::channel(8);
    let mut mouse_down = captured(1, InputPhase::Down, 20);
    // 即使 UIA 检查开始太晚，之前冻结的图像仍属于这个鼠标事件。
    mouse_down.captured_at -= Duration::from_secs(1);
    let (saved, screenshot) = tokio::sync::oneshot::channel();
    saved
        .send(Ok(ScreenshotEvidence {
            path: "evidence/1.png".into(),
            captured_at_ms: 1,
            capture_duration_ms: 1,
            screen_bounds: context().bounds,
            width: 2000,
            height: 1000,
            pointer: Some(ScreenPoint { x: 20, y: 20 }),
            crop: None,
            crop_failure: None,
        }))
        .unwrap();
    mouse_down.screenshot = Some(screenshot);
    let mut clipboard = captured(2, InputPhase::Up, 20);
    clipboard.event.input = PhysicalInput::Clipboard { sequence_number: 7 };
    clipboard.clipboard = Some(ClipboardContent::Text {
        value: "copied contents".into(),
        truncated: false,
    });
    let mut window = captured(3, InputPhase::Up, 20);
    window.event.input = PhysicalInput::Window {
        window: context().window,
        change: WindowChange::Foreground,
    };
    // WinEvent 的历史时间早于先送达的剪贴板消息，最终按事件时间排序。
    window.elapsed_ms = 1;
    let mut wheel = captured(5, InputPhase::Up, 20);
    wheel.event.input = PhysicalInput::Wheel {
        point: ScreenPoint { x: 20, y: 20 },
        delta: -120,
        horizontal: false,
    };
    for input in [mouse_down, clipboard, window, wheel] {
        sender.send(input).await.ok().unwrap();
    }
    drop(sender);
    let calls = Arc::new(AtomicUsize::new(0));
    let trace = crate::worker::record(
        receiver,
        resolver(calls.clone()),
        Arc::new(AtomicU64::new(1)),
        uuid::Uuid::new_v4(),
        0,
        Arc::new(AtomicU64::new(0)),
    )
    .await;
    assert_eq!(
        trace
            .timeline
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        [1, 3, 2, 5]
    );
    let evidence = trace.timeline.events[0].evidence.as_ref().unwrap();
    assert!(evidence.ui_snapshot.is_none());
    assert_eq!(evidence.screenshot.as_ref().unwrap().captured_at_ms, 1);
    assert!(
        matches!(&trace.timeline.events[2].input, RawInput::Clipboard { content: ClipboardContent::Text { value, .. }, .. } if value == "copied contents")
    );
    assert!(
        trace.timeline.events[3]
            .diagnostics
            .contains(&RecordingDiagnostic::InputGap)
    );
    assert_eq!(trace.dropped_events, 1);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}
