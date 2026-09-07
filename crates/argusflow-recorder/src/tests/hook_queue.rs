use super::*;

#[test]
fn full_hook_queue_drops_without_blocking_and_preserves_sequence_gap() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let dropped = Arc::new(AtomicU64::new(0));
    SINK.with(|sink| {
        *sink.borrow_mut() = Some(HookSink {
            sender,
            sequence: 0,
            dropped: dropped.clone(),
        })
    });
    let input = PhysicalInput::Move {
        point: argusflow_core::ScreenPoint { x: 5, y: 6 },
    };
    emit(1, input);
    emit(2, input);
    assert_eq!(receiver.try_recv().unwrap().sequence, 1);
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    emit(3, input);
    assert_eq!(receiver.try_recv().unwrap().sequence, 3);
    SINK.with(|sink| *sink.borrow_mut() = None);
}
