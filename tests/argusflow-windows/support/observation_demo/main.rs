//! 未知操作观察原型；采集、示范和离线推导具有独立入口。
mod actor;
#[path = "../wechat_demo/application.rs"]
#[allow(dead_code)]
mod application;
mod browser;
mod chat;
mod clipboard;
mod editor_state;
mod focus;
mod inspect_chat;
#[path = "../patchright_demo/native.rs"]
#[allow(dead_code)]
mod native;
#[path = "../patchright_recording_demo/notepad.rs"]
#[allow(dead_code)]
mod notepad;
mod observe;
mod samples;
mod screen;
mod send;
mod send_command;
mod send_transition;
#[path = "../patchright_recording_demo/taskbar.rs"]
mod taskbar;
#[path = "../patchright_recording_demo/wechat_support.rs"]
#[allow(dead_code)]
mod wechat_support;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    match args.as_slice() {
        [mode, output] if mode=="inspect-chat" => inspect_chat::run(&root,std::path::Path::new(output)).await,
        [mode, recipient, expected] if mode=="paste-send" => send_command::run(&root,recipient,expected).await,
        [mode] if mode=="contact-check" => chat::check(&root).await,
        [mode, directory, seconds] if mode == "observe" => observe::run(std::path::Path::new(directory), seconds.parse()?, &root).await,
        [mode, directory] if mode == "demonstrate" => actor::run(std::path::Path::new(directory), &root, None).await,
        [mode, directory, workflow] if mode == "replay" => actor::run(std::path::Path::new(directory), &root, Some(std::path::Path::new(workflow))).await,
        _ => Err("用法：observation_demo observe <新目录> <秒数> | demonstrate <目录> | replay <新目录> <推导的workflow>".into()),
    }
}
