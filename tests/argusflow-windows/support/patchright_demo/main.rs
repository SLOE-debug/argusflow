//! 独立 demo：现有 Windows 能力管理进程和窗口，Node 负责浏览器。
mod bridge;
mod native;
mod report;

use std::{error::Error, path::PathBuf};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/argusflow-windows/support/patchright_demo")
        .canonicalize()?;
    let chrome = native::find_chrome()?;
    println!("发现 Chrome：{}", chrome.display());
    let report = bridge::run(&directory, &chrome).await?;
    let output = report::save(&directory, &report)?;
    println!(
        "已保存 {} 条百度热搜：{}",
        report.items.len(),
        output.display()
    );
    Ok(())
}
