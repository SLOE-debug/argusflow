//! 保留 AI 流程的动作顺序，绑定实时列表参数、执行并记录结果。
use super::{Result, native, notepad::Notepad};
use argusflow_browser::{Browser, BrowserConfig, LaunchOptions};
use argusflow_core::OperationOptions;
use argusflow_runtime::{NodeRegistry, RunInputs, RunOptions, RunStatus, WorkflowEngine, prepare};
use argusflow_windows::{ClipboardReader, WindowLocator};
use argusflow_workflow::{Value, Workflow};
use argusflow_workflow_automation::{
    AutomationHost, PageResource, WindowResource, register_automation,
};
use std::{path::PathBuf, sync::Arc, time::Duration};

pub(super) async fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [workflow, source, output] = args.as_slice() else {
        return Err("需要 workflow.json、--baidu 或源 HTML、新输出目录".into());
    };
    let mut definition = Workflow::from_json(&std::fs::read_to_string(workflow)?)?;
    let output = PathBuf::from(output);
    std::fs::create_dir(&output)?;
    let output = output.canonicalize()?;
    let file = output.join(format!(
        "{}.txt",
        output
            .file_name()
            .ok_or("输出目录缺少文件名")?
            .to_string_lossy()
    ));
    let marker = format!("ArgusFlow Replay {}", std::process::id());
    let browser = Browser::launch(
        LaunchOptions::new(native::find_chrome()?),
        BrowserConfig::default(),
    )
    .await?;
    let outcome=async {
        let url=if source=="--baidu" { "https://www.baidu.com/".to_owned() } else {
            let source=PathBuf::from(source).canonicalize()?;
            format!("file:///{}",source.display().to_string().trim_start_matches(r"\\?\").replace('\\',"/"))
        };
        let page=browser.new_page(&url,OperationOptions::default()).await?;
        let mut live_inputs=std::collections::BTreeMap::new();
        if source=="--baidu" {
            let list=super::baidu::discover(&page,&output).await?;
            live_inputs=super::live_binding::bind(&mut definition,&list)?;
            std::fs::write(output.join("workflow.json"),definition.to_json()?)?;
            std::fs::write(output.join("live-inputs.json"),serde_json::to_vec_pretty(&live_inputs)?)?;
            println!("真实百度前3名：{:?}",list.items.iter().take(3).map(|i|&i.title).collect::<Vec<_>>());
        }
        page.evaluate(&format!("document.title={}",serde_json::to_string(&marker)?),OperationOptions::default()).await?;
        let browser_window=tokio::time::timeout(Duration::from_secs(10),async {
            loop {
                let windows:Vec<_>=WindowLocator{class_name:Some("Chrome_WidgetWin_1".into()),..Default::default()}.find_all()?.into_iter().filter(|w|w.title().starts_with(&marker)).collect();
                if let [window]=windows.as_slice(){return Ok::<_,Box<dyn std::error::Error>>(window.identity());}
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }).await??;
        let editor=Notepad::open(&file).await.map_err(|e|format!("准备记事本：{e}"))?;
        let clipboard=ClipboardReader::start()?;
        let result=async {
            let initial=editor.read_text(super::notepad::EDITOR_QUERY).await.map_err(|e|format!("读取初始文档：{e}"))?;
            if !initial.is_empty(){return Err("初始文档不是空白".into());}
            let host=AutomationHost{uia:Some(editor.runtime().clone()),input:Some(editor.input()),clipboard:Some(clipboard.clone()),..Default::default()};
            let mut registry=NodeRegistry::new();register_automation(&mut registry,host)?;super::chat_task::register(&mut registry,Some(output.clone()))?;
            let plan=prepare(definition,&registry).map_err(|e|format!("{e:?}"))?;
            let mut bindings=RunInputs::default();
            bindings.values.extend(live_inputs);
            bindings.values.insert("output_path".into(),Value::Text(file.display().to_string()));
            bindings.resources.insert("browser_page".into(),Arc::new(PageResource::attached(page)));
            bindings.resources.insert("browser_window".into(),Arc::new(WindowResource::borrowed(browser_window)));
            bindings.resources.insert("editor_window".into(),Arc::new(WindowResource::borrowed(editor.window.clone())));
            let engine=WorkflowEngine::new();
            let mut handle=engine.start(plan,bindings,RunOptions{max_steps:300,..Default::default()})?;
            let mut events=handle.subscribe();
            let log_path=output.join("events.jsonl");
            let logger=tokio::spawn(async move {
                use std::io::Write;
                let mut file=std::fs::File::create(log_path).map_err(|e|e.to_string())?;
                loop {
                    let event=events.recv().await;
                    let text=format!("{event:?}");
                    writeln!(file,"{}",serde_json::json!({"event":text})).map_err(|e|e.to_string())?;
                    println!("{text}");
                    if text.contains("RunFinished")||text=="Closed" {break;}
                }
                Ok::<_,String>(())
            });
            let result=tokio::time::timeout(Duration::from_secs(180),handle.wait()).await;
            let result=match result {Ok(value)=>value?,Err(error)=>{handle.cancel();let _=handle.wait().await;return Err(error.into());}};
            logger.await??;
            let final_text=editor.read_text(super::notepad::EDITOR_QUERY).await?;
            let final_clipboard=clipboard.observe(None,OperationOptions::default()).await?;
            let report=serde_json::json!({"status":format!("{:?}",result.status),"error":result.error.as_ref().map(|e|format!("{e:?}")),"document":final_text,"clipboard":final_clipboard,"saved_file":file,"disk_text":std::fs::read_to_string(&file)?});
            std::fs::write(output.join("result.json"),serde_json::to_vec_pretty(&report)?)?;
            if result.status!=RunStatus::Completed{return Err(format!("回放失败：{:?}",result.error).into());}
            Ok::<_,Box<dyn std::error::Error>>(())
        }.await;
        let clip_cleanup=clipboard.shutdown(OperationOptions::default()).await;
        let editor_cleanup=editor.shutdown().await;
        result?;clip_cleanup?;editor_cleanup?;
        Ok::<_,Box<dyn std::error::Error>>(())
    }.await;
    let cleanup = browser.shutdown(OperationOptions::default()).await;
    outcome?;
    cleanup?;
    Ok(())
}
