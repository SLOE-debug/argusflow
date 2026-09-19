//! 显式验收入口：只在新建文件和独立浏览器中运行 AI 输出的正式流程。
mod baidu;
#[path = "../chat_task.rs"]
mod chat_task;
mod live_binding;
#[path = "../../../argusflow-windows/support/patchright_demo/native.rs"]
#[allow(dead_code)]
mod native;
#[path = "../../../argusflow-windows/support/patchright_recording_demo/notepad.rs"]
#[allow(dead_code)]
mod notepad;
mod runner;
#[path = "../../../argusflow-windows/support/patchright_recording_demo/taskbar.rs"]
mod taskbar;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[tokio::main]
async fn main() -> Result<()> {
    runner::run().await
}
