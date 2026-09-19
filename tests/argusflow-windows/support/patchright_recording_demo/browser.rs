//! 复用 Application 的浏览器侧车，一次会话处理多个逐条读取请求。
use super::Result;
use argusflow_core::{Operation, OperationOptions};
use argusflow_windows::{Application, ApplicationOptions};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
};

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request<'a> {
    Open { url: &'a str },
    Read { selector: &'a str, index: usize },
    Close,
}
#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Ready,
    Opened { url: String },
    Item { title: String, url: String },
    Closed,
    Error { message: String },
}
pub struct Browser {
    application: Application,
    stream: BufReader<NamedPipeServer>,
    profile: PathBuf,
}
impl Browser {
    pub async fn start(runtime: &Path, run: &Path, marker: &str) -> Result<Self> {
        let pipe_name = format!(r"\\.\pipe\{marker}");
        let pipe = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_name)?;
        let node = std::env::var_os("PATH")
            .and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|p| p.join("node.exe"))
                    .find(|p| p.is_file())
            })
            .ok_or("找不到 node.exe")?
            .canonicalize()?;
        let profile = run.join("chrome-profile");
        std::fs::create_dir(&profile)?;
        let mut options = ApplicationOptions::new(node);
        options.visible = false;
        options.arguments = vec![
            runtime.join("recording-driver.mjs").display().to_string(),
            pipe_name,
            super::native::find_chrome()?.display().to_string(),
            profile.display().to_string(),
            marker.into(),
        ];
        let application =
            Application::launch(options, &Operation::new(OperationOptions::default()))?;
        tokio::time::timeout(Duration::from_secs(15), pipe.connect()).await??;
        let mut browser = Self {
            application,
            stream: BufReader::new(pipe),
            profile,
        };
        match browser.receive().await? {
            Response::Ready => {}
            other => return Err(format!("浏览器未就绪：{other:?}").into()),
        }
        super::native::inspect_window(marker).await?;
        Ok(browser)
    }
    async fn receive(&mut self) -> Result<Response> {
        let mut line = String::new();
        let size = tokio::time::timeout(
            Duration::from_secs(40),
            (&mut self.stream).take(65_537).read_line(&mut line),
        )
        .await??;
        if size == 0 || size > 65_536 || !line.ends_with('\n') {
            return Err("浏览器响应关闭或超限".into());
        }
        let response = serde_json::from_str(&line)?;
        match response {
            Response::Error { message } => Err(message.into()),
            response => Ok(response),
        }
    }
    pub async fn request(&mut self, request: Request<'_>) -> Result<Response> {
        let mut bytes = serde_json::to_vec(&request)?;
        bytes.push(b'\n');
        self.stream.get_mut().write_all(&bytes).await?;
        self.receive().await
    }
    pub async fn shutdown(&mut self) -> Result<()> {
        let close = self.request(Request::Close).await;
        self.application
            .shutdown(&Operation::new(OperationOptions::default()))
            .await?;
        // 目录由本次独占 run 目录创建，只删除本次浏览器临时配置。
        std::fs::remove_dir_all(&self.profile)?;
        match close? {
            Response::Closed => Ok(()),
            _ => Err("浏览器未确认关闭".into()),
        }
    }
}
