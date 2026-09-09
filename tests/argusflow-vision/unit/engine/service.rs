use super::*;
use std::sync::atomic::AtomicUsize;
fn options(ms: u64) -> OperationOptions {
    OperationOptions::new(Duration::from_millis(ms)).unwrap()
}
fn input() -> ImageInput {
    ImageInput::Encoded(vec![1])
}

#[tokio::test]
async fn cancelled_native_work_blocks_admission_and_drops_late_results() {
    let config = OcrConfig {
        queue_capacity: 1,
        ..OcrConfig::new("unused")
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    let (ready, ready_rx) = oneshot::channel();
    let (entered, entered_rx) = oneshot::channel();
    let (release, wait) = mpsc::channel();
    let shared = Arc::new(Shared {
        state: Mutex::new(OcrState::Loading),
        active: Mutex::new(None),
        run: Arc::default(),
        stopping: AtomicBool::new(false),
    });
    let worker_shared = shared.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let worker_calls = calls.clone();
    let thread = std::thread::spawn(move || {
        let mut entered = Some(entered);
        worker_with(
            receiver,
            ready,
            worker_shared,
            Operation::new(options(2000)),
            || Ok(()),
            move |_, _, _, _| {
                worker_calls.fetch_add(1, Ordering::SeqCst);
                if let Some(entered) = entered.take() {
                    entered.send(()).unwrap();
                    wait.recv().unwrap();
                }
                Ok(OcrResult {
                    blocks: Vec::new(),
                    width: 1,
                    height: 1,
                })
            },
        );
    });
    ready_rx.await.unwrap().unwrap();
    let engine = OcrEngine {
        inner: Arc::new(Inner {
            sender,
            shared,
            thread: Mutex::new(Some(thread)),
            config,
        }),
    };
    let first = engine.clone();
    let task =
        tokio::spawn(async move { first.recognize_with_options(input(), options(300)).await });
    entered_rx.await.unwrap();
    let second = engine.clone();
    let queued =
        tokio::spawn(async move { second.recognize_with_options(input(), options(1000)).await });
    tokio::task::yield_now().await;
    assert_eq!(
        engine.recognize(input()).await.unwrap_err().kind(),
        FailureKind::Busy
    );
    queued.abort();
    let _ = queued.await;
    assert_eq!(
        task.await.unwrap().unwrap_err().kind(),
        FailureKind::Timeout
    );
    assert_eq!(engine.state(), OcrState::Unresponsive);
    assert_eq!(
        engine.recognize(input()).await.unwrap_err().kind(),
        FailureKind::Unresponsive
    );
    assert_eq!(
        engine.shutdown(options(20)).await.unwrap_err().kind(),
        FailureKind::Unresponsive
    );
    release.send(()).unwrap();
    let (a, b) = tokio::join!(
        engine.shutdown(options(1000)),
        engine.shutdown(options(1000))
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(engine.state(), OcrState::Stopped);
}
