use super::*;

#[test]
fn full_hook_queue_drops_without_blocking_and_preserves_sequence_gap() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let dropped = Arc::new(AtomicU64::new(0));
    SINK.with(|sink| {
        *sink.borrow_mut() = Some(HookSink {
            wake: None,
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

#[test]
fn system_notifications_share_the_input_sequence_without_becoming_actions() {
    let (sender, receiver) = mpsc::sync_channel(4);
    SINK.with(|sink| {
        *sink.borrow_mut() = Some(HookSink {
            wake: None,
            sender,
            sequence: 0,
            dropped: Arc::new(AtomicU64::new(0)),
        })
    });
    emit(
        10,
        PhysicalInput::Window {
            window: argusflow_core::WindowIdentity {
                handle: 42,
                process_id: 7,
            },
            change: crate::WindowChange::Appeared,
        },
    );
    emit(11, PhysicalInput::Clipboard { sequence_number: 9 });
    let window = receiver.try_recv().unwrap();
    let clipboard = receiver.try_recv().unwrap();
    assert_eq!((window.sequence, clipboard.sequence), (1, 2));
    assert!(matches!(
        window.input,
        PhysicalInput::Window {
            change: crate::WindowChange::Appeared,
            ..
        }
    ));
    assert!(matches!(
        clipboard.input,
        PhysicalInput::Clipboard { sequence_number: 9 }
    ));
    SINK.with(|sink| *sink.borrow_mut() = None);
}
