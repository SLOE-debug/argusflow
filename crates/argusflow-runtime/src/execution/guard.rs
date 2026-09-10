//! 扩展 Future 的 panic 不能让运行永远保持 Running。
use crate::RunError;
use argusflow_workflow::ErrorKind;
use std::{
    future::{Future, poll_fn},
    panic::{AssertUnwindSafe, catch_unwind},
    task::Poll,
};
pub(crate) async fn guard_future<T>(
    future: impl Future<Output = Result<T, RunError>>,
) -> Result<T, RunError> {
    let mut future = std::pin::pin!(future);
    poll_fn(
        move |context| match catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(context))) {
            Ok(poll) => poll,
            Err(_) => Poll::Ready(Err(RunError::new(
                ErrorKind::Contract,
                "扩展执行意外 panic，运行已终止",
            ))),
        },
    )
    .await
}
