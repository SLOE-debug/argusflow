//! 有界 JSON Lines 命名管道；复用 Application 管理 Node 和 Chrome 进程树。
use super::{native, report::Report};
use argusflow_core::{Operation, OperationOptions};
use argusflow_windows::{Application, ApplicationOptions};
use serde::Deserialize;
use std::{
    error::Error,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::windows::named_pipe::ServerOptions,
};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Message {
    Ready,
    Result { report: Report },
    Error { message: String },
}

/// 单次运行只有一个 Node 进程和一个独立 Chrome 配置目录。
pub async fn run(directory: &Path, chrome: &Path) -> Result<Report, Box<dyn Error>> {
    // 唯一标记同时隔离命名管道、窗口标题和 Chrome 配置目录。
    let marker = format!(
        "ArgusFlow-Patchright-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    );
    let pipe_name = format!(r"\\.\pipe\{marker}");
    let pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe_name)?;
    let node = std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|path| path.join("node.exe"))
                .find(|path| path.is_file())
        })
        .ok_or("PATH 中未找到 node.exe")?
        .canonicalize()?;
    let profile = directory.join("output").join(&marker);
    std::fs::create_dir_all(&profile)?;
    let mut options = ApplicationOptions::new(node);
    options.visible = false;
    options.arguments = [
        directory.join("driver.mjs").to_string_lossy().into_owned(),
        pipe_name,
        chrome.to_string_lossy().into_owned(),
        profile.to_string_lossy().into_owned(),
        marker.clone(),
    ]
    .into();
    let application = Application::launch(options, &Operation::new(OperationOptions::default()))?;
    let result = tokio::time::timeout(Duration::from_secs(90), async {
        tokio::time::timeout(Duration::from_secs(15), pipe.connect()).await??;
        let mut stream = BufReader::new(pipe);
        match receive(&mut stream).await? {
            Message::Ready => {}
            Message::Error { message } => return Err(message.into()),
            Message::Result { .. } => return Err("Node 协议错误：尚未就绪".into()),
        }
        println!("UIA 确认窗口：{}", native::inspect_window(&marker).await?);
        stream
            .get_mut()
            .write_all(b"{\"type\":\"scrape\"}\n")
            .await?;
        match receive(&mut stream).await? {
            Message::Result { report } => Ok(report),
            Message::Error { message } => Err(message.into()),
            Message::Ready => Err("Node 协议错误：重复就绪".into()),
        }
    })
    .await;
    // 正常和失败路径都终止并回收仅属于本次 demo 的完整 Job。
    let cleanup = application
        .shutdown(&Operation::new(OperationOptions::new(
            Duration::from_secs(10),
        )?))
        .await;
    cleanup?;
    std::fs::remove_dir_all(&profile)?;
    result?
}

/// 单条消息最多 1 MiB，防止意外输出无限占用内存。
async fn receive<R: tokio::io::AsyncRead + Unpin>(
    stream: &mut BufReader<R>,
) -> Result<Message, Box<dyn Error>> {
    let mut line = String::new();
    let size = stream.take(1_048_577).read_line(&mut line).await?;
    if size == 0 || size > 1_048_576 || !line.ends_with('\n') {
        return Err("Node 管道关闭或响应消息超出限制".into());
    }
    Ok(serde_json::from_str(&line)?)
}
