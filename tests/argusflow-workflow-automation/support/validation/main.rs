//! 录制模型产物的只读契约验证入口，不启动执行引擎。
#[path = "../chat_task.rs"]
mod chat_task;
mod request;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    request::run()
}
