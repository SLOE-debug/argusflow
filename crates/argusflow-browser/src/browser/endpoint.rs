//! 调试端点发现和自建 Chromium 的启动准备。
use crate::BrowserError as Failure;
use crate::browser::{BrowserConfig, LaunchOptions};
use argusflow_core::{FailureKind, Operation};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::process::{Child, Command};
use url::Url;

#[derive(Deserialize)]
struct Version {
    #[serde(rename = "webSocketDebuggerUrl")]
    websocket: String,
}
pub(crate) async fn resolve(
    endpoint: &str,
    operation: &Operation,
    config: &BrowserConfig,
) -> Result<String, Failure> {
    let url =
        Url::parse(endpoint).map_err(|error| invalid("调试端点不是绝对 URL").with_source(error))?;
    match url.scheme() {
        "ws" | "wss" => Ok(url.into()),
        "http" | "https" => {
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(operation.remaining())
                .build()
                .map_err(|error| invalid("无法创建端点查询连接").with_source(error))?;
            let version = url
                .join("/json/version")
                .map_err(|error| invalid("调试端点路径无效").with_source(error))?;
            let mut response = client
                .get(version)
                .send()
                .await
                .map_err(|error| {
                    Failure::new(
                        FailureKind::Unavailable,
                        "cdp_discovery",
                        "无法查询调试端点",
                    )
                    .with_source(error)
                })?
                .error_for_status()
                .map_err(|error| invalid("调试端点返回失败状态").with_source(error))?;
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|error| invalid("调试端点响应读取失败").with_source(error))?
            {
                operation.check("cdp_discovery")?;
                if bytes.len() + chunk.len() > config.max_message_bytes {
                    return Err(invalid("调试端点响应过大"));
                }
                bytes.extend_from_slice(&chunk);
            }
            let version: Version = serde_json::from_slice(&bytes).map_err(|error| {
                invalid("调试端点缺少 Browser WebSocket 地址").with_source(error)
            })?;
            let websocket = Url::parse(&version.websocket)
                .map_err(|error| invalid("WebSocket 地址无效").with_source(error))?;
            if !matches!(websocket.scheme(), "ws" | "wss") {
                return Err(invalid("调试端点返回非 WebSocket 地址"));
            }
            Ok(websocket.into())
        }
        _ => Err(invalid("端点只支持 HTTP(S) 或 WS(S)")),
    }
}

pub(crate) fn launch(options: &LaunchOptions, profile: &Path) -> Result<Child, Failure> {
    if !options.executable.is_absolute() || !options.executable.is_file() {
        return Err(invalid("浏览器可执行文件必须为存在的绝对路径"));
    }
    let mut command = Command::new(&options.executable);
    command
        .arg("--remote-debugging-address=127.0.0.1")
        .arg("--remote-debugging-port=0")
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("about:blank")
        .kill_on_drop(true);
    if options.headless {
        command.arg("--headless=new");
    }
    command.spawn().map_err(|error| {
        Failure::new(FailureKind::Unavailable, "browser_launch", "无法启动浏览器")
            .with_source(error)
    })
}
pub(crate) async fn wait_endpoint(
    child: &mut Child,
    profile: &Path,
    operation: &Operation,
) -> Result<String, Failure> {
    let port_file = profile.join("DevToolsActivePort");
    loop {
        operation.check("browser_launch")?;
        if child
            .try_wait()
            .map_err(|error| invalid("无法查询浏览器进程").with_source(error))?
            .is_some()
        {
            return Err(Failure::new(
                FailureKind::Unavailable,
                "browser_launch",
                "浏览器在调试端点就绪前退出",
            ));
        }
        if let Ok(contents) = tokio::fs::read_to_string(&port_file).await {
            let mut lines = contents.lines();
            if let (Some(port), Some(path)) = (
                lines.next().and_then(|port| port.parse::<u16>().ok()),
                lines.next(),
            ) && port != 0
                && path.starts_with("/devtools/browser/")
            {
                return Ok(format!("ws://127.0.0.1:{port}{path}"));
            }
        }
        tokio::time::sleep(operation.remaining().min(Duration::from_millis(25))).await;
    }
}
pub(crate) fn validate_url(url: &str) -> Result<(), Failure> {
    if url.len() > 1_048_576 {
        return Err(invalid("导航地址超过 1 MiB"));
    }
    let parsed =
        Url::parse(url).map_err(|error| invalid("导航地址必须是绝对 URL").with_source(error))?;
    if !matches!(
        parsed.scheme(),
        "http" | "https" | "file" | "about" | "data"
    ) {
        return Err(invalid("不支持此导航 URL 协议"));
    }
    Ok(())
}
pub(crate) fn profile_root() -> PathBuf {
    std::env::temp_dir().join("argusflow-browser")
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailureKind::InvalidInput, "cdp_endpoint", message)
}
