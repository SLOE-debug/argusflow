//! 文件能力边界：独占创建与受取消、大小预算约束的只读确认。
use argusflow_core::Operation;
use argusflow_runtime::RunError;
use argusflow_workflow::ErrorKind;
use std::time::Duration;
use tokio::io::AsyncReadExt;

pub(super) async fn create_new(path: &str, operation: &Operation) -> Result<(), RunError> {
    operation.begin_effect("file_create_new")?;
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await
        .map_err(io)?;
    drop(file);
    Ok(())
}
pub(super) async fn wait_text(
    path: &str,
    expected: &str,
    operation: &Operation,
) -> Result<(), RunError> {
    if expected.len() > 1024 * 1024 {
        return Err(RunError::new(
            ErrorKind::Contract,
            "文件校验文字不能超过 1 MiB",
        ));
    }
    loop {
        operation.check("file_wait_text")?;
        let size = tokio::fs::metadata(path).await.map_err(io)?.len();
        if size == expected.len() as u64 {
            let file = tokio::fs::File::open(path).await.map_err(io)?;
            let mut bytes = Vec::new();
            file.take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .await
                .map_err(io)?;
            operation.check("file_wait_text_read")?;
            if bytes == expected.as_bytes() {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(100).min(operation.remaining())).await;
    }
}
fn io(error: std::io::Error) -> RunError {
    RunError::new(ErrorKind::Unavailable, error.to_string())
}
