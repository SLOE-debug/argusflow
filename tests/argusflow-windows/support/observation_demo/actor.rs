//! 独立示范者/回放执行者；不写事件证据，不参与离线推导。
use super::{Result, browser::Browser, clipboard, notepad::Notepad};
use argusflow_core::{Key, OperationOptions};
use argusflow_windows::{InputAction, WindowLocator};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};
async fn pause() -> Result<()> {
    tokio::time::sleep(Duration::from_millis(1800)).await;
    Ok(())
}
async fn keys(editor: &Notepad, keys: Vec<Key>) -> Result<()> {
    editor
        .input()
        .perform(
            editor.window.clone(),
            InputAction::Chord(keys),
            OperationOptions::default(),
        )
        .await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    Ok(())
}
pub async fn run(directory: &Path, root: &Path, workflow: Option<&Path>) -> Result<()> {
    std::fs::create_dir_all(directory)?;
    let plan: Value = if let Some(path) = workflow {
        let value: Value = serde_json::from_slice(&std::fs::read(path)?)?;
        if value["format"] != "observation-demo-v1"
            || value["unresolved"].as_array().is_none_or(|v| !v.is_empty())
        {
            return Err("推导存在未解决项，拒绝回放".into());
        }
        value
    } else {
        json!({"steps":[
        {"action":"browser_copy","url":"https://www.baidu.com/","selector":"#hotsearch-content-wrapper li .title-content-title","index":1},
        {"action":"document_paste"},{"action":"browser_copy","url":"https://www.baidu.com/","selector":"#hotsearch-content-wrapper li .title-content-title","index":2},
        {"action":"document_paste"},{"action":"browser_copy","url":"https://www.baidu.com/","selector":"#hotsearch-content-wrapper li .title-content-title","index":3},
        {"action":"document_paste"},{"action":"document_copy","line":0},{"action":"chat_paste_send","recipient":"文件传输助手"},
        {"action":"document_copy","line":1},{"action":"chat_paste_send","recipient":"文件传输助手"},
        {"action":"document_copy","line":2},{"action":"chat_paste_send","recipient":"文件传输助手"}]})
    };
    let marker = format!("Observed-{}", super::samples::epoch_ms());
    let runtime = root.join("tests/argusflow-windows/support/patchright_demo");
    let mut browser = Browser::start(&runtime, directory, &marker).await?;
    let mut notepad: Option<Notepad> = None;
    let mut url = String::new();
    let result=async {
        for step in plan["steps"].as_array().ok_or("步骤缺失")? {
            println!("执行 {}",step["action"]);
            match step["action"].as_str().ok_or("动作缺失")? {
                "browser_copy"=>{
                    let target=step["url"].as_str().ok_or("URL缺失")?;
                    if target!="https://www.baidu.com/" && !target.starts_with("file:///") {return Err("demo 仅支持百度或本地测试页面".into());}
                    if url!=target {browser.request(json!({"type":"open","url":target})).await?;url=target.into();}
                    let title=if target.starts_with("file:///") {"ArgusFlow Evidence - Google Chrome"} else {"百度一下，你就知道 - Google Chrome"};
                    let window=WindowLocator{title:Some(title.into()),class_name:Some("Chrome_WidgetWin_1".into()),..Default::default()}.find_unique()?.identity();
                    if let Some(editor)=&notepad {super::focus::chrome(editor.runtime(),&editor.input(),&window).await?;}
                    else {let runtime=argusflow_windows::UiaRuntime::start(Default::default(),OperationOptions::default()).await?;let input=argusflow_windows::InputService::new()?;let result=super::focus::chrome(&runtime,&input,&window).await;input.shutdown(OperationOptions::default()).await?;runtime.shutdown(OperationOptions::default()).await?;result?;}
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    let sequence=clipboard::sequence();
                    browser.request(json!({"type":"copy","selector":step["selector"],"index":step["index"].as_u64().unwrap_or(0)})).await?;
                    if sequence==clipboard::sequence(){return Err("复制后剪贴板未更新，拒绝粘贴旧内容".into());}
                },
                "document_paste"=>{
                    if notepad.is_none(){notepad=Some(Notepad::open(&directory.join(format!("{marker}.txt"))).await?);}
                    let editor=notepad.as_ref().ok_or("记事本未打开")?;
                    let before=editor.read_text(super::notepad::EDITOR_QUERY).await?;
                    let text=clipboard::read()?;if text.trim().is_empty(){return Err("浏览器复制没有文字".into());}
                    keys(editor,vec![Key::Control,Key::End]).await?;pause().await?;
                    keys(editor,vec![Key::Control,Key::Letter('V')]).await?;pause().await?;
                    keys(editor,vec![Key::Enter]).await?;
                    keys(editor,vec![Key::Control,Key::Letter('S')]).await?;pause().await?;
                    let after=editor.read_text(super::notepad::EDITOR_QUERY).await?;
                    if after.trim_end()!=format!("{before}{text}").trim_end(){return Err("真实粘贴内容不一致".into());}
                },
                "document_copy"=>{
                    let editor=notepad.as_ref().ok_or("记事本未打开")?;
                    editor.read_text(super::notepad::EDITOR_QUERY).await?;
                    keys(editor,vec![Key::Control,Key::Home]).await?;
                    for _ in 0..step["line"].as_u64().ok_or("行号缺失")?{keys(editor,vec![Key::Down]).await?;}
                    keys(editor,vec![Key::Home]).await?;keys(editor,vec![Key::Shift,Key::End]).await?;
                    keys(editor,vec![Key::Shift,Key::Left]).await?;pause().await?;
                    keys(editor,vec![Key::Control,Key::Letter('C')]).await?;pause().await?;
                },
                "chat_paste_send"=>super::send::send(notepad.as_ref().ok_or("记事本未打开")?,root,step["recipient"].as_str().ok_or("接收人缺失")?).await?,
                _=>return Err("推导动作尚无执行器".into()),
            }
        }
        Ok::<(),Box<dyn std::error::Error>>(())
    }.await;
    if let Some(editor) = notepad {
        editor.shutdown().await?;
    }
    browser.shutdown().await?;
    result
}
