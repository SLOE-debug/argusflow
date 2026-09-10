//! 故障注入证明动作不重放、参数无效不触发 I/O、取消和部分执行可观察。
use super::*;
use argusflow_aql::{Bindings, compile};
use argusflow_core::{Effect, OperationOptions};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
struct Backend {
    found: usize,
    calls: AtomicUsize,
    clicks: AtomicUsize,
    fail: bool,
    cancel: bool,
    text: Mutex<String>,
    ids: Mutex<Vec<u64>>,
}
impl Backend {
    fn new(found: usize) -> Self {
        Self {
            found,
            calls: AtomicUsize::new(0),
            clicks: AtomicUsize::new(0),
            fail: false,
            cancel: false,
            text: Mutex::new("已有".into()),
            ids: Mutex::new(Vec::new()),
        }
    }
}
impl SourceBackend for Backend {
    type Target = usize;
    fn find<'a>(
        &'a self,
        _: &'a BoundQuery,
        op: &'a Operation,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<usize>, Failure>> + Send + 'a>>
    {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.ids.lock().unwrap().push(op.id());
            if self.cancel {
                op.cancel();
            }
            Ok((0..self.found).collect())
        })
    }
    fn click<'a>(
        &'a self,
        _: &'a usize,
        op: &'a Operation,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Failure>> + Send + 'a>> {
        Box::pin(async move {
            op.begin_effect("fake_click")?;
            self.clicks.fetch_add(1, Ordering::SeqCst);
            self.ids.lock().unwrap().push(op.id());
            if self.fail {
                Err(Failure::new(FailureKind::Timeout, "fake_click", "响应丢失"))
            } else {
                Ok(())
            }
        })
    }
    fn type_text<'a>(
        &'a self,
        _: &'a usize,
        text: &'a str,
        op: &'a Operation,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Failure>> + Send + 'a>> {
        Box::pin(async move {
            op.begin_effect("fake_text")?;
            self.text.lock().unwrap().push_str(text);
            Ok(())
        })
    }
}
fn query() -> BoundQuery {
    compile("button()").unwrap().bind(&Bindings::new()).unwrap()
}
#[tokio::test]
async fn unique_and_fresh_location() {
    for count in [0, 2] {
        let backend = Backend::new(count);
        let error = execute(
            &backend,
            &query(),
            Action::Click,
            &Operation::new(OperationOptions::default()),
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.kind(),
            if count == 0 {
                FailureKind::NotFound
            } else {
                FailureKind::Ambiguous
            }
        );
        assert_eq!(backend.clicks.load(Ordering::SeqCst), 0);
    }
    let backend = Backend::new(1);
    for _ in 0..2 {
        execute(
            &backend,
            &query(),
            Action::Click,
            &Operation::new(OperationOptions::default()),
        )
        .await
        .unwrap();
    }
    assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
    let ids = backend.ids.lock().unwrap();
    assert_eq!(ids[0], ids[1]);
    assert_eq!(ids[2], ids[3]);
}
#[tokio::test]
async fn partial_failure_never_replays_and_insertion_preserves_content() {
    let mut backend = Backend::new(1);
    backend.fail = true;
    let error = execute(
        &backend,
        &query(),
        Action::Click,
        &Operation::new(OperationOptions::default()),
    )
    .await
    .unwrap_err();
    assert_eq!(error.effect(), Effect::Unconfirmed);
    assert_eq!(backend.clicks.load(Ordering::SeqCst), 1);
    execute(
        &backend,
        &query(),
        Action::TypeText("追加"),
        &Operation::new(OperationOptions::default()),
    )
    .await
    .unwrap();
    assert_eq!(*backend.text.lock().unwrap(), "已有追加");
}
#[tokio::test]
async fn cancellation_and_validation_prevent_actions() {
    let mut backend = Backend::new(1);
    backend.cancel = true;
    assert_eq!(
        execute(
            &backend,
            &query(),
            Action::Click,
            &Operation::new(OperationOptions::default())
        )
        .await
        .unwrap_err()
        .kind(),
        FailureKind::Cancelled
    );
    assert_eq!(backend.clicks.load(Ordering::SeqCst), 0);
    let backend = Backend::new(1);
    assert!(
        execute(
            &backend,
            &query(),
            Action::TypeText(""),
            &Operation::new(OperationOptions::default())
        )
        .await
        .is_err()
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
}
