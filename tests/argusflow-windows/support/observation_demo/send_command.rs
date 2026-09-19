//! 引擎示例调用的受限入口：只允许已授权文件助手，正文必须来自当前剪贴板。
use super::Result;
use argusflow_core::OperationOptions;
use argusflow_windows::InputService;
pub async fn run(root: &std::path::Path, recipient: &str, expected: &str) -> Result<()> {
    if super::clipboard::read()? != expected {
        return Err("剪贴板与 AI 节点的预期正文不符，未粘贴发送".into());
    }
    let input = InputService::new()?;
    let runtime =
        argusflow_windows::UiaRuntime::start(Default::default(), OperationOptions::default())
            .await?;
    let result = super::send::send_with_services(&runtime, &input, root, recipient, expected).await;
    let input_cleanup = input.shutdown(OperationOptions::default()).await;
    let uia_cleanup = runtime.shutdown(OperationOptions::default()).await;
    result?;
    input_cleanup?;
    uia_cleanup?;
    Ok(())
}
