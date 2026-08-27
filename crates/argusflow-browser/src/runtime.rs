//! 隔离 Chromium 进程、随机调试端口与 CDP page session 生命周期。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use argusflow_core::{
    AcquireBrowserSpec, BrowserAcquireMode, BrowserCleanupPolicy, BrowserError, BrowserSession,
    BrowserSessionProvider, ResourceId,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{
    process::{Child, Command},
    time::Instant,
};
use uuid::Uuid;

use crate::cdp::{CdpConnection, CdpPageSession, CdpSessionRegistry};

/// 应用级 CDP runtime，同时提供 Browser 资源生命周期和后端会话注册表。
#[derive(Debug, Default)]
pub struct CdpRuntime {
    /// ActionBackend prepare 的同步只读会话快路径。
    pub(crate) sessions: Arc<CdpSessionRegistry>,
    /// 只保存 ArgusFlow 本次启动的浏览器进程和隔离目录。
    browsers: Mutex<HashMap<ResourceId, ManagedBrowser>>,
}

impl CdpRuntime {
    /// 创建空的应用级 CDP runtime。
    pub fn new() -> Self {
        Self::default()
    }
}

/// 资源表之外由 provider 独占的不可克隆进程状态。
#[derive(Debug)]
struct ManagedBrowser {
    /// 浏览器根进程句柄。
    child: Child,
    /// Chromium 本次运行的隔离用户目录。
    profile_directory: PathBuf,
}

#[async_trait]
impl BrowserSessionProvider for CdpRuntime {
    async fn acquire(&self, spec: &AcquireBrowserSpec) -> Result<BrowserSession, BrowserError> {
        validate_spec(spec)?;
        let resource_id = ResourceId::new();
        let profile_directory = create_profile_directory(resource_id).await?;
        let mut child = launch_browser(spec, &profile_directory)?;
        let process_id = child.id().ok_or_else(|| BrowserError::LaunchFailed {
            message: "browser process did not expose a process id".to_owned(),
        })?;
        let acquisition = acquire_page_session(spec, &profile_directory, &mut child).await;
        let page_session = match acquisition {
            Ok(session) => session,
            Err(error) => {
                terminate_child(&mut child).await;
                let _ = tokio::fs::remove_dir_all(&profile_directory).await;
                return Err(error);
            }
        };
        let target_id = page_session.target_id().to_owned();
        self.sessions.insert(resource_id, page_session);
        self.browsers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                resource_id,
                ManagedBrowser {
                    child,
                    profile_directory,
                },
            );
        Ok(BrowserSession {
            id: resource_id,
            spec: spec.clone(),
            process_id,
            target_id,
        })
    }

    async fn navigate(&self, session: &BrowserSession, url: &str) -> Result<(), BrowserError> {
        if !is_http_url(url) {
            return Err(BrowserError::InvalidSpec {
                message: "navigation URL must be an absolute HTTP(S) URL".to_owned(),
            });
        }
        let page = self
            .sessions
            .get(session.id)
            .ok_or_else(|| BrowserError::NavigationFailed {
                message: "browser session is no longer attached".to_owned(),
            })?;
        let response = page
            .command("Page.navigate", json!({ "url": url }))
            .await
            .map_err(navigation_error)?;
        if let Some(error_text) = response.get("errorText").and_then(Value::as_str) {
            return Err(BrowserError::NavigationFailed {
                message: error_text.to_owned(),
            });
        }
        wait_for_document_ready(&page, session.spec.launch_timeout_ms).await
    }

    async fn cleanup(&self, session: &BrowserSession) -> Result<(), BrowserError> {
        let page_session = self.sessions.remove(session.id);
        if let Some(page_session) = page_session {
            let _ = page_session.close_browser().await;
        }
        let managed = self
            .browsers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&session.id);
        let Some(mut managed) = managed else {
            return Ok(());
        };
        wait_then_terminate_child(&mut managed.child).await;
        tokio::fs::remove_dir_all(&managed.profile_directory)
            .await
            .map_err(|error| BrowserError::CleanupFailed {
                message: format!(
                    "failed to remove isolated browser profile '{}': {error}",
                    managed.profile_directory.display(),
                ),
            })
    }
}

/// 在启动前建立路径、URL 和超时不变量。
fn validate_spec(spec: &AcquireBrowserSpec) -> Result<(), BrowserError> {
    let executable = Path::new(spec.executable_path.trim());
    if !executable.is_absolute() || !executable.is_file() {
        return Err(BrowserError::InvalidSpec {
            message: "browser executable must be an existing absolute file".to_owned(),
        });
    }
    if !matches!(spec.acquire_mode, BrowserAcquireMode::LaunchIsolatedCdp) {
        return Err(BrowserError::InvalidSpec {
            message: "unsupported browser acquire mode".to_owned(),
        });
    }
    if !matches!(
        spec.cleanup_policy,
        BrowserCleanupPolicy::CloseOnWorkflowEnd
    ) {
        return Err(BrowserError::InvalidSpec {
            message: "unsupported browser cleanup policy".to_owned(),
        });
    }
    if !(100..=60_000).contains(&spec.launch_timeout_ms) {
        return Err(BrowserError::InvalidSpec {
            message: "launch_timeout_ms must be between 100 and 60000".to_owned(),
        });
    }
    Ok(())
}

/// 创建位于系统临时目录下且名称不可碰撞的隔离用户目录。
async fn create_profile_directory(resource_id: ResourceId) -> Result<PathBuf, BrowserError> {
    let directory = std::env::temp_dir().join("argusflow-cdp").join(format!(
        "{}-{}",
        Uuid::new_v4(),
        resource_id_hash(resource_id)
    ));
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| BrowserError::LaunchFailed {
            message: format!(
                "failed to create isolated browser profile '{}': {error}",
                directory.display(),
            ),
        })?;
    Ok(directory)
}

/// ResourceId 的调试后缀只用于目录可读性，不参与资源身份。
fn resource_id_hash(resource_id: ResourceId) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    resource_id.hash(&mut hasher);
    hasher.finish()
}

/// 使用随机调试端口和隔离 profile 直接启动 Chromium 系浏览器。
fn launch_browser(
    spec: &AcquireBrowserSpec,
    profile_directory: &Path,
) -> Result<Child, BrowserError> {
    let mut command = Command::new(&spec.executable_path);
    command
        .arg("--remote-debugging-address=127.0.0.1")
        .arg("--remote-debugging-port=0")
        .arg(format!("--user-data-dir={}", profile_directory.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-background-networking")
        .arg("about:blank")
        .kill_on_drop(true);
    command.spawn().map_err(|error| BrowserError::LaunchFailed {
        message: format!("failed to start '{}': {error}", spec.executable_path),
    })
}

/// 等待 DevToolsActivePort、建立根连接、发现页面并创建扁平 target session。
async fn acquire_page_session(
    spec: &AcquireBrowserSpec,
    profile_directory: &Path,
    child: &mut Child,
) -> Result<Arc<CdpPageSession>, BrowserError> {
    let deadline = Instant::now() + Duration::from_millis(spec.launch_timeout_ms);
    let web_socket_url =
        wait_for_debug_endpoint(profile_directory, child, deadline, spec.launch_timeout_ms).await?;
    let connection = CdpConnection::connect(&web_socket_url)
        .await
        .map_err(protocol_launch_error)?;
    connection
        .command(
            None,
            "Target.setDiscoverTargets",
            json!({ "discover": true }),
        )
        .await
        .map_err(protocol_launch_error)?;
    let target_id = wait_for_page_target(&connection, deadline, spec.launch_timeout_ms).await?;
    CdpPageSession::attach(connection, target_id)
        .await
        .map_err(protocol_launch_error)
}

/// 读取 Chromium 原子写入的端口和 browser WebSocket path。
async fn wait_for_debug_endpoint(
    profile_directory: &Path,
    child: &mut Child,
    deadline: Instant,
    timeout_ms: u64,
) -> Result<String, BrowserError> {
    let active_port_file = profile_directory.join("DevToolsActivePort");
    loop {
        if let Ok(contents) = tokio::fs::read_to_string(&active_port_file).await {
            let mut lines = contents.lines();
            let port = lines.next().and_then(|line| line.parse::<u16>().ok());
            let path = lines.next().filter(|line| line.starts_with('/'));
            if let (Some(port), Some(path)) = (port, path) {
                return Ok(format!("ws://127.0.0.1:{port}{path}"));
            }
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| BrowserError::LaunchFailed {
                message: format!("failed to observe browser process: {error}"),
            })?
        {
            return Err(BrowserError::LaunchFailed {
                message: format!(
                    "browser exited with {status} before publishing DevToolsActivePort"
                ),
            });
        }
        if Instant::now() >= deadline {
            return Err(BrowserError::Timeout { timeout_ms });
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 轮询 Target.getTargets，优先选择与初始地址同 host 的普通 page。
async fn wait_for_page_target(
    connection: &CdpConnection,
    deadline: Instant,
    timeout_ms: u64,
) -> Result<String, BrowserError> {
    loop {
        let response = connection
            .command(None, "Target.getTargets", json!({}))
            .await
            .map_err(protocol_launch_error)?;
        let targets = response
            .get("targetInfos")
            .and_then(Value::as_array)
            .ok_or_else(|| BrowserError::LaunchFailed {
                message: "Target.getTargets did not return targetInfos".to_owned(),
            })?;
        let page = targets
            .iter()
            .filter_map(|value| serde_json::from_value::<TargetInfo>(value.clone()).ok())
            .find(|target| target.target_type == "page");
        if let Some(page) = page {
            return Ok(page.target_id);
        }
        if Instant::now() >= deadline {
            return Err(BrowserError::Timeout { timeout_ms });
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 等待导航后的主文档进入 interactive/complete，复用资源启动超时作为明确边界。
async fn wait_for_document_ready(
    page: &CdpPageSession,
    timeout_ms: u64,
) -> Result<(), BrowserError> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let response = page
            .command(
                "Runtime.evaluate",
                json!({ "expression": "document.readyState", "returnByValue": true }),
            )
            .await
            .map_err(navigation_error)?;
        let ready_state = response.pointer("/result/value").and_then(Value::as_str);
        if matches!(ready_state, Some("interactive" | "complete")) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(BrowserError::Timeout { timeout_ms });
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Target.getTargets 中当前执行器需要的字段。
#[derive(Debug, Deserialize)]
struct TargetInfo {
    /// CDP target ID。
    #[serde(rename = "targetId")]
    target_id: String,
    /// `page`、`worker` 等 target 类型。
    #[serde(rename = "type")]
    target_type: String,
}

/// 尽力终止并回收本次启动的根进程。
async fn terminate_child(child: &mut Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

/// 优雅关闭后短暂等待；仍存活时才强制终止根进程。
async fn wait_then_terminate_child(child: &mut Child) {
    if tokio::time::timeout(Duration::from_secs(2), child.wait())
        .await
        .is_ok()
    {
        return;
    }
    terminate_child(child).await;
}

/// 内部协议错误转换为浏览器获取错误。
fn protocol_launch_error(error: impl std::fmt::Display) -> BrowserError {
    BrowserError::LaunchFailed {
        message: error.to_string(),
    }
}

/// 把页面级协议错误映射到明确的导航错误边界。
fn navigation_error(error: impl std::fmt::Display) -> BrowserError {
    BrowserError::NavigationFailed {
        message: error.to_string(),
    }
}

/// 判断字符串是否是当前资源允许的网络页面地址。
fn is_http_url(value: &str) -> bool {
    value.split_once("://").is_some_and(|(scheme, remainder)| {
        matches!(scheme, "http" | "https") && !remainder.trim().is_empty()
    })
}
