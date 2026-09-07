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

fn resolver(calls: Arc<AtomicUsize>) -> Arc<TargetResolver> {
    let inspector = Arc::new(Inspector(calls));
    Arc::new(TargetResolver::new(
        Arc::new(Window),
        inspector.clone(),
        inspector.clone(),
        inspector,
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
            .raw
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert_eq!(trace.normalized.records.len(), 2);
    assert!(
        trace.normalized.records[0]
            .target
            .entity
            .as_ref()
            .unwrap()
            .identity
            .contains("x: 1")
    );
    assert!(
        trace.normalized.records[1]
            .target
            .entity
            .as_ref()
            .unwrap()
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
        assert_eq!(
            trace.normalized.records[0].target.backend,
            ResolutionBackend::Coordinate
        );
        if late {
            assert_eq!(calls.load(Ordering::Relaxed), 0);
        }
    }
}
