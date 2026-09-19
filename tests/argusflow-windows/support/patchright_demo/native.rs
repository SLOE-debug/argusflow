//! 本机安装发现，以及复用现有窗口/UIA 的原生窗口检查。
use argusflow_windows::{
    OperationOptions, Predicate, Query, SearchScope, UiaConfig, UiaRuntime, WindowLocator,
};
use std::{error::Error, path::PathBuf, time::Duration};

/// 按当前用户安装、系统安装顺序定位现有 Chrome，不下载浏览器。
pub fn find_chrome() -> Result<PathBuf, Box<dyn Error>> {
    for variable in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(variable) {
            let executable = PathBuf::from(root).join("Google/Chrome/Application/chrome.exe");
            if executable.is_file() {
                return Ok(executable.canonicalize()?);
            }
        }
    }
    Err("未在当前用户或系统标准安装目录找到 Google Chrome".into())
}

/// 只检查本次 demo 的唯一标题窗口，不读取用户已有标签页。
pub async fn inspect_window(marker: &str) -> Result<String, Box<dyn Error>> {
    let options = OperationOptions::new(Duration::from_secs(10))?;
    let window = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let mut matches: Vec<_> = WindowLocator {
                class_name: Some("Chrome_WidgetWin_1".into()),
                ..Default::default()
            }
            .find_all()?
            .into_iter()
            .filter(|window| window.title().starts_with(marker))
            .collect();
            match matches.len() {
                1 => return Ok::<_, Box<dyn Error>>(matches.remove(0)),
                0 => tokio::time::sleep(Duration::from_millis(100)).await,
                _ => return Err("demo 窗口标题不唯一".into()),
            }
        }
    })
    .await??;
    let runtime = UiaRuntime::start(UiaConfig::default(), options).await?;
    let result = async {
        let element = runtime
            .find_unique(
                Query {
                    window: window.identity(),
                    predicate: Predicate::Any,
                    scope: SearchScope::Root,
                },
                options,
            )
            .await?;
        let snapshot = runtime.read(&element, options).await;
        element.release();
        Ok::<_, Box<dyn Error>>(snapshot?.name)
    }
    .await;
    let cleanup = runtime.shutdown(options).await;
    let title = result?;
    cleanup?;
    Ok(title)
}
