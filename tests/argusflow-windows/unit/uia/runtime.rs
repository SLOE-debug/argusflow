use super::*;
use argusflow_core::Effect;
use std::sync::{atomic::AtomicUsize, mpsc};

struct Controlled {
    gate: mpsc::Receiver<()>,
    entered: Option<oneshot::Sender<()>>,
    calls: Arc<AtomicUsize>,
    panic: bool,
}
impl worker::Backend for Controlled {
    fn prune(&mut self) {}
    fn handle(&mut self, _: worker::Command, operation: &Operation) -> Result<Response, Failure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(entered) = self.entered.take() {
            operation.begin_effect("fake_provider")?;
            let _ = entered.send(());
            self.gate.recv().unwrap();
        }
        assert!(!self.panic, "controlled provider failure");
        Ok(Response::Done)
    }
}
fn controlled(
    panic: bool,
) -> (
    UiaRuntime,
    mpsc::Sender<()>,
    oneshot::Receiver<()>,
    Arc<AtomicUsize>,
) {
    let (sender, receiver) = mpsc::sync_channel(1);
    let (gate, wait) = mpsc::channel();
    let (entered, observed) = oneshot::channel();
    // run_backend waits for a live startup receiver; preserve it on the worker stack.
    let (ready, ready_rx) = oneshot::channel();
    let shared = Arc::new(Shared {
        state: Mutex::new(UiaState::Ready),
        active: Mutex::new(None),
        stopping: AtomicBool::new(false),
        finished: Notify::new(),
    });
    let calls = Arc::new(AtomicUsize::new(0));
    let backend = Controlled {
        gate: wait,
        entered: Some(entered),
        calls: calls.clone(),
        panic,
    };
    let worker_shared = shared.clone();
    let thread = std::thread::spawn(move || {
        let _ready_rx = ready_rx;
        worker::run_backend(receiver, ready, worker_shared, || Ok(backend));
    });
    (
        UiaRuntime {
            inner: Arc::new(Inner {
                sender,
                shared,
                thread: Mutex::new(Some(thread)),
                id: 1,
            }),
        },
        gate,
        observed,
        calls,
    )
}
fn short(ms: u64) -> OperationOptions {
    OperationOptions::new(Duration::from_millis(ms)).unwrap()
}

fn probe() -> worker::Command {
    worker::Command::Find(
        Query {
            window: crate::WindowIdentity::test_identity(),
            predicate: crate::Predicate::Any,
            scope: crate::SearchScope::Root,
        },
        false,
    )
}

#[tokio::test]
async fn foreign_and_revoked_leases_fail_before_native_dispatch() {
    let (runtime, _, _, calls) = controlled(false);
    let element = ElementHandle {
        runtime: 2,
        id: 1,
        window: crate::WindowIdentity::test_identity(),
        lease: Arc::new(super::super::element::Lease {
            alive: AtomicBool::new(true),
        }),
    };
    assert_eq!(
        runtime.read(&element, short(100)).await.unwrap_err().kind(),
        FailureKind::StaleHandle
    );
    let mut own = element.clone();
    own.runtime = 1;
    own.release();
    assert_eq!(
        runtime.read(&own, short(100)).await.unwrap_err().kind(),
        FailureKind::StaleHandle
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    runtime.shutdown(short(1000)).await.unwrap();
}

#[tokio::test]
async fn queue_full_cancelled_queue_and_late_effect_are_bounded() {
    let (runtime, gate, entered, calls) = controlled(false);
    let first = runtime.clone();
    let active = tokio::spawn(async move { first.call(probe(), short(300)).await });
    entered.await.unwrap();
    let second = runtime.clone();
    let queued = tokio::spawn(async move { second.call(probe(), short(1000)).await });
    tokio::task::yield_now().await;
    assert_eq!(
        runtime
            .call(probe(), short(1000))
            .await
            .err()
            .unwrap()
            .kind(),
        FailureKind::Busy
    );
    queued.abort();
    let _ = queued.await;
    let error = active.await.unwrap().err().unwrap();
    assert_eq!(error.kind(), FailureKind::Timeout);
    assert_eq!(error.effect(), Effect::Unconfirmed);
    assert_eq!(runtime.state(), UiaState::Unresponsive);
    assert_eq!(
        runtime.call(probe(), short(20)).await.err().unwrap().kind(),
        FailureKind::Unresponsive
    );
    assert_eq!(
        runtime.shutdown(short(20)).await.unwrap_err().kind(),
        FailureKind::Unresponsive
    );
    assert_eq!(runtime.state(), UiaState::Stopping);
    gate.send(()).unwrap();
    runtime.shutdown(short(1000)).await.unwrap();
    assert_eq!(runtime.state(), UiaState::Stopped);
    assert_eq!(calls.load(Ordering::SeqCst), 1); // 排队取消没有到达 Provider，副作用没有重试。
}

#[tokio::test]
async fn provider_panic_and_concurrent_shutdown_do_not_hang() {
    let (runtime, gate, entered, _) = controlled(true);
    let first = runtime.clone();
    let active = tokio::spawn(async move { first.call(probe(), short(1000)).await });
    entered.await.unwrap();
    gate.send(()).unwrap();
    assert!(active.await.unwrap().is_err());
    let (a, b) = tokio::join!(runtime.shutdown(short(1000)), runtime.shutdown(short(1000)));
    a.unwrap();
    b.unwrap();
    assert_eq!(runtime.state(), UiaState::Failed);
}
