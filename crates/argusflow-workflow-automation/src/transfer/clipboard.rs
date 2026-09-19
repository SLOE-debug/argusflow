//! 剪贴板序号与正文共同验证，不因相同文本重复出现而认定新复制完成。
use crate::AutomationHost;
use argusflow_core::OperationOptions;
use argusflow_input_contracts::{ClipboardContent, ClipboardObservation};
use argusflow_runtime::{RunError, TaskContext};
use argusflow_workflow::ErrorKind;
use std::time::Duration;

pub(super) async fn read(
    host: &AutomationHost,
    context: &TaskContext<'_>,
) -> Result<ClipboardObservation, RunError> {
    context.operation.check("clipboard_checkpoint")?;
    let reader = host
        .clipboard
        .as_ref()
        .ok_or_else(|| RunError::new(ErrorKind::Unavailable, "剪贴板服务未装配"))?;
    Ok(reader
        .observe(
            None,
            OperationOptions::new(context.operation.remaining().min(Duration::from_secs(2)))?,
        )
        .await
        .map_err(argusflow_core::Failure::from)?)
}
pub(super) async fn wait(
    host: &AutomationHost,
    context: &TaskContext<'_>,
    previous: u32,
    expected: &str,
) -> Result<ClipboardObservation, RunError> {
    loop {
        let observation = read(host, context).await?;
        if observation.sequence != previous {
            if matches!(&observation.content, ClipboardContent::Text{text,truncated:false} if text==expected)
            {
                return Ok(observation);
            }
            return Err(RunError::new(
                ErrorKind::Operation,
                "剪贴板改变但正文不符，停止传输",
            ));
        }
        tokio::time::sleep(Duration::from_millis(30).min(context.operation.remaining())).await;
    }
}
