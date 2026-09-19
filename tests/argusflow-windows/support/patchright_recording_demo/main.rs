//! 浏览器、Windows 记事本和微信的独立录制/回放 demo。
#[path = "../wechat_demo/application.rs"]
mod application;
mod browser;
mod evidence;
mod model;
#[path = "../patchright_demo/native.rs"]
mod native;
mod notepad;
mod raw;
mod runner;
mod taskbar;
mod wechat;
mod wechat_once;
#[allow(dead_code)]
mod wechat_support;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/argusflow-windows/support/patchright_recording_demo")
        .canonicalize()?;
    match args.as_slice() {
        [mode] if mode == "record" => {
            runner::run(&directory, model::Workflow::demonstration(), true).await
        }
        [mode, path] if mode == "replay" => {
            let workflow = serde_json::from_slice(&std::fs::read(path)?)?;
            runner::run(&directory, workflow, false).await
        }
        [mode] if mode == "notepad-check" => runner::check_notepad(&directory).await,
        [mode, text] if mode == "clear-demo-draft" => wechat::clear_own_draft(text, &directory.join("../../../..").canonicalize()?).await,
        _ => Err(
            "用法：patchright_recording_demo record | replay <workflow.json> | notepad-check | clear-demo-draft <完整草稿>"
                .into(),
        ),
    }
}
