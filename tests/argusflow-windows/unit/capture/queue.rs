use super::*;
#[test]
fn overflow_preserves_source_identity_and_reports_gap_before_resuming() {
    let queue = EventQueue::new(1024);
    let source = SourceInfo {
        id: SourceId(1),
        name: "test".into(),
        generation: 1,
        bounds: ScreenRect::new(0, 0, 16, 16).unwrap(),
        rotation: Rotation::Identity,
        dpi: (96, 96),
        state: SourceState::Ready,
        failure: None,
    };
    queue.push(BackendEvent::Source(source), ClockTime(1));
    for time in 2..100 {
        queue.push(
            BackendEvent::Watermark {
                source: SourceId(1),
                generation: 1,
                through: ClockTime(time),
            },
            ClockTime(time),
        );
    }
    let events = queue.drain();
    assert!(matches!(events.first(), Some(BackendEvent::Source(_))));
    assert!(
        events
            .iter()
            .any(|event| matches!(event,BackendEvent::Gap(gap) if gap.reason==GapReason::Capacity))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, BackendEvent::Watermark { .. }))
    );
    assert!(queue.drain().is_empty());
    queue.push(
        BackendEvent::Watermark {
            source: SourceId(1),
            generation: 1,
            through: ClockTime(100),
        },
        ClockTime(100),
    );
    assert_eq!(queue.drain().len(), 1);
}
