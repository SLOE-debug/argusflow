use crate::*;
use argusflow_core::{ScreenPoint, WindowIdentity};

fn movement(sequence: u64, elapsed_ms: u64, x: i32, y: i32) -> RawTraceEvent {
    RawTraceEvent {
        sequence,
        timestamp_ms: elapsed_ms as u32,
        elapsed_ms,
        input: RawInput::Move {
            point: ScreenPoint { x, y },
        },
        evidence: None,
        diagnostics: vec![],
    }
}

fn compact(events: Vec<RawTraceEvent>) -> EventTimeline {
    let mut timeline = EventTimeline { events };
    timeline.compact_pointer_motion();
    timeline
}

fn motion(event: &RawTraceEvent) -> &PointerMotion {
    let RawInput::PointerMotion(motion) = &event.input else {
        panic!("expected a movement segment")
    };
    motion
}

#[test]
fn dense_straight_movement_becomes_one_event_with_timed_anchors() {
    let timeline = compact(
        (0..1738)
            .map(|index| movement(index + 1, index, index as i32 - 2000, -100))
            .collect(),
    );
    assert_eq!(timeline.events.len(), 1);
    let segment = motion(&timeline.events[0]);
    assert_eq!(segment.sample_count, 1738);
    assert_eq!(segment.end_sequence, 1738);
    assert_eq!(segment.ended_ms, 1737);
    assert_eq!(segment.distance_px, 1737.0);
    assert_eq!(
        segment.points.first().unwrap().point,
        ScreenPoint { x: -2000, y: -100 }
    );
    assert_eq!(
        segment.points.last().unwrap().point,
        ScreenPoint { x: -263, y: -100 }
    );
    assert!(segment.points.len() <= 9);
    let bytes = serde_json::to_vec(&timeline).unwrap();
    assert!(
        bytes.len() < 2000,
        "a straight movement must not export thousands of sampling objects"
    );
    let mut roundtrip: EventTimeline = serde_json::from_slice(&bytes).unwrap();
    roundtrip.compact_pointer_motion();
    assert_eq!(
        timeline, roundtrip,
        "opening an existing recording must be idempotent"
    );
    let trace = RecordingTrace {
        schema_version: 2,
        recording_id: uuid::Uuid::new_v4(),
        started_at_unix_ms: 0,
        timeline,
        dropped_events: 0,
    };
    let summary = RecordingSummary::from_trace(&trace);
    assert_eq!((summary.event_count, summary.duration_ms), (1, 1737));
}

#[test]
fn corners_reversals_and_return_to_origin_are_preserved() {
    let coordinates = [
        (0, 0),
        (50, 0),
        (100, 0),
        (100, 50),
        (100, 100),
        (100, 50),
        (100, 0),
        (50, 0),
        (0, 0),
    ];
    let timeline = compact(
        coordinates
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| movement(index as u64 + 1, index as u64 * 10, x, y))
            .collect(),
    );
    let segment = motion(&timeline.events[0]);
    assert_eq!(segment.distance_px, 400.0);
    assert!(
        segment
            .points
            .iter()
            .any(|sample| sample.point == ScreenPoint { x: 100, y: 100 })
    );
    assert_eq!(
        segment.points.first().unwrap().point,
        segment.points.last().unwrap().point
    );
    assert!(
        segment.points.len() >= 5,
        "a closed drag must not collapse into an unmoving point"
    );
}

#[test]
fn every_non_movement_event_and_diagnostic_boundary_separates_segments() {
    for input in [
        RawInput::Mouse {
            point: ScreenPoint { x: 3, y: 0 },
            button: MouseButton::Left,
            phase: InputPhase::Down,
        },
        RawInput::Window {
            window: WindowIdentity {
                handle: 1,
                process_id: 2,
            },
            change: WindowChange::Foreground,
        },
        RawInput::Window {
            window: WindowIdentity {
                handle: 1,
                process_id: 2,
            },
            change: WindowChange::Appeared,
        },
        RawInput::Clipboard {
            sequence_number: 3,
            content: ClipboardContent::Empty,
        },
        RawInput::Wheel {
            point: ScreenPoint { x: 3, y: 0 },
            delta: 120,
            horizontal: false,
        },
        super::fixtures::text("boundary"),
    ] {
        let barrier = super::fixtures::raw(3, input);
        let timeline = compact(vec![
            movement(1, 10, 0, 0),
            movement(2, 20, 1, 0),
            barrier.clone(),
            movement(4, 40, 3, 0),
            movement(5, 50, 4, 0),
        ]);
        assert_eq!(timeline.events.len(), 3);
        assert_eq!(
            timeline.events[1], barrier,
            "all evidence and input on the boundary must remain unchanged"
        );
    }
    let mut late = movement(3, 30, 2, 0);
    late.diagnostics.push(RecordingDiagnostic::LateInspection);
    let mut evidence = movement(4, 40, 3, 0);
    evidence.evidence = Some(super::fixtures::target());
    let timeline = compact(vec![
        movement(1, 10, 0, 0),
        movement(2, 20, 1, 0),
        late.clone(),
        evidence.clone(),
    ]);
    assert_eq!(
        timeline.events,
        [timeline.events[0].clone(), late, evidence]
    );
}

#[test]
fn pause_source_gap_and_late_window_notification_do_not_merge() {
    let timeline = compact(vec![
        movement(1, 0, 0, 0),
        movement(2, 10, 1, 0),
        movement(3, 211, 2, 0),
        movement(5, 212, 3, 0),
    ]);
    assert_eq!(timeline.events.len(), 3);
    assert_eq!(motion(&timeline.events[0]).sample_count, 2);
    let mut late_window = movement(5, 25, 0, 0);
    late_window.input = RawInput::Window {
        window: WindowIdentity {
            handle: 1,
            process_id: 1,
        },
        change: WindowChange::Foreground,
    };
    let timeline = compact(vec![
        movement(1, 10, 0, 0),
        movement(2, 20, 1, 0),
        movement(3, 30, 2, 0),
        movement(4, 40, 3, 0),
        late_window,
    ]);
    assert_eq!(
        timeline
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        [1, 5, 3]
    );
    assert_eq!(motion(&timeline.events[2]).sample_count, 2);
}

#[test]
fn held_button_path_is_distinct_from_free_movement_and_gap_revokes_held_state() {
    let mut down = movement(1, 0, 0, 0);
    down.input = RawInput::Mouse {
        point: ScreenPoint { x: 0, y: 0 },
        button: MouseButton::Left,
        phase: InputPhase::Down,
    };
    let mut up = movement(4, 30, 20, 0);
    up.input = RawInput::Mouse {
        point: ScreenPoint { x: 20, y: 0 },
        button: MouseButton::Left,
        phase: InputPhase::Up,
    };
    let timeline = compact(vec![
        down.clone(),
        movement(2, 10, 10, 0),
        movement(3, 20, 20, 0),
        up,
        movement(5, 40, 30, 0),
        movement(6, 50, 40, 0),
    ]);
    assert_eq!(
        motion(&timeline.events[1]).pressed_buttons,
        [MouseButton::Left]
    );
    assert!(motion(&timeline.events[3]).pressed_buttons.is_empty());
    let timeline = compact(vec![down, movement(3, 10, 10, 0), movement(4, 20, 20, 0)]);
    assert!(motion(&timeline.events[1]).pressed_buttons.is_empty());
}

#[test]
fn long_and_high_rate_runs_are_bounded_without_losing_sources() {
    for events in [
        (0..9000)
            .map(|index| movement(index + 1, 1, 0, 0))
            .collect::<Vec<_>>(),
        (0..101)
            .map(|index| movement(index + 1, index * 100, index as i32, 0))
            .collect(),
    ] {
        let count = events.len();
        let timeline = compact(events);
        assert!(timeline.events.len() < 4);
        let retained: usize = timeline
            .events
            .iter()
            .map(|event| match &event.input {
                RawInput::PointerMotion(segment) => segment.sample_count,
                _ => 1,
            })
            .sum();
        assert_eq!(retained, count);
        for event in &timeline.events {
            if let RawInput::PointerMotion(segment) = &event.input {
                assert!(segment.sample_count <= 4096);
                assert!(segment.ended_ms - event.elapsed_ms <= 5000);
            }
        }
    }
}

#[tokio::test]
async fn saved_unmerged_recording_is_compacted_on_open_without_rewriting_source() {
    let id = uuid::Uuid::new_v4();
    let root = std::env::temp_dir().join(format!("argusflow-motion-test-{id}"));
    let trace = RecordingTrace {
        schema_version: 2,
        recording_id: id,
        started_at_unix_ms: 0,
        timeline: EventTimeline {
            events: (0..1000)
                .map(|index| movement(index + 1, index, index as i32, 0))
                .collect(),
        },
        dropped_events: 0,
    };
    let files = crate::storage::save(&root, &trace).await.unwrap();
    let before = tokio::fs::read(&files.timeline).await.unwrap();
    let loaded = crate::history::load(&root, id).await.unwrap();
    assert_eq!(loaded.trace.timeline.events.len(), 1);
    assert_eq!(crate::history::list(&root).await.unwrap()[0].event_count, 1);
    assert_eq!(tokio::fs::read(&files.timeline).await.unwrap(), before);
    for path in [&files.timeline, &files.manifest] {
        tokio::fs::remove_file(path).await.unwrap();
    }
    tokio::fs::remove_dir(&files.evidence_directory)
        .await
        .unwrap();
    tokio::fs::remove_dir(root.join(id.to_string()))
        .await
        .unwrap();
    tokio::fs::remove_dir(root).await.unwrap();
}
