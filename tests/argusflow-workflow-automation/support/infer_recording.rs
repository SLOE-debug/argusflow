//! 正式 AI 模块的最小调用示例；配置仅来自 SQLite，不加载 .env。
use argusflow_ai::{CancellationToken, ConfigStore, Evidence, analyze};
use std::{path::Path, sync::Arc};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [database, recording, destination] = args.as_slice() else {
        return Err("需要 SQLite 配置路径、正式录制目录、全新结果文件路径".into());
    };
    let (config, key) = ConfigStore::open(Path::new(database))?.load()?;
    let evidence = Arc::new(Evidence::load(Path::new(recording))?);
    let mut registry = argusflow_runtime::NodeRegistry::new();
    argusflow_workflow_automation::register_automation(&mut registry, Default::default())?;
    let result = analyze(
        config,
        key,
        evidence,
        &registry,
        CancellationToken::new(),
        |event| {
            if let Ok(text) = serde_json::to_string(&event) {
                eprintln!("{text}");
            }
        },
    )
    .await?;
    use std::io::Write;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    output.write_all(&serde_json::to_vec_pretty(&result)?)?;
    output.sync_all()?;
    Ok(())
}
