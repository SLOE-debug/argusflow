//! 默认使用本地协议替身；真实浏览器只有明确传入 --browser 才启动。
#[path = "../cdp.rs"]
mod cdp;
mod lifecycle_model;
use argusflow_runtime::*;
use argusflow_workflow_automation::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let value = |flag: &str| {
        arguments
            .iter()
            .position(|arg| arg == flag)
            .and_then(|index| arguments.get(index + 1))
            .map(String::as_str)
    };
    let application = value("--application");
    let browser = value("--browser");
    if arguments.iter().any(|arg| arg == "--json") {
        println!(
            "{}",
            lifecycle_model::document(
                "ws://127.0.0.1:9222/devtools/browser/explicit",
                application,
                browser
            )
            .to_json()?
        );
        return Ok(());
    }
    let server = if browser.is_none() {
        Some(cdp::Server::start().await)
    } else {
        None
    };
    let endpoint = server
        .as_ref()
        .map_or("", |server| server.endpoint.as_str());
    let definition = lifecycle_model::document(endpoint, application, browser);
    let mut registry = NodeRegistry::new();
    register_automation(&mut registry, AutomationHost::default())?;
    let plan = prepare(definition, &registry).map_err(|errors| format!("{errors:#?}"))?;
    let engine = WorkflowEngine::new();
    let mut handle = engine.start(plan, RunInputs::default(), RunOptions::default())?;
    let result = handle.wait().await?;
    if let Some(error) = &result.error {
        return Err(error.clone().into());
    }
    if engine.retained_resources().await != 0 {
        return Err("资源清理未完成".into());
    }
    if let Some(server) = server {
        println!(
            "本地 CDP 替身处理 {} 条命令",
            server.calls.lock().map_err(|_| "fixture lock")?.len()
        );
    }
    println!("异常已捕获；作用域资源全部释放");
    Ok(())
}
