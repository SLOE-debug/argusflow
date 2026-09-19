//! 执行动作、记录真实结果，并从成功动作生成可再次执行的 workflow。
use super::{
    Result,
    browser::{Browser, Request, Response},
    model::{Step, Workflow},
    notepad::Notepad,
    raw::RawRecording,
    wechat,
};
use std::{
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

/// 原型录制是动作级录制；系统 Hook 另存原始输入证据。
pub async fn run(directory: &Path, workflow: Workflow, record: bool) -> Result<()> {
    workflow.validate()?;
    let marker = format!(
        "ArgusFlow-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    );
    let run = directory.join("output").join(&marker);
    std::fs::create_dir_all(&run)?;
    println!("本次输出：{}", run.display());
    let root = directory.join("../../../..").canonicalize()?;
    let runtime = directory.join("../patchright_demo").canonicalize()?;
    let mut browser = Browser::start(&runtime, &run, &marker).await?;
    let mut notepad = None;
    let mut recording = None;
    let mut trace = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(run.join("actions.jsonl"))?;
    let mut captured = Workflow {
        format: workflow.format.clone(),
        steps: Vec::new(),
    };
    let result = execute(
        &workflow,
        &mut captured,
        &mut browser,
        &mut notepad,
        &mut recording,
        &root,
        &run,
        &marker,
        &mut trace,
    )
    .await;
    let raw_result = recording.map(RawRecording::finish).transpose();
    let notepad_cleanup = if let Some(notepad) = &notepad {
        notepad.shutdown().await
    } else {
        Ok(())
    };
    let browser_cleanup = browser.shutdown().await;
    if let Err(error) = &result {
        writeln!(
            trace,
            "{}",
            serde_json::json!({"status":"failed", "error":error.to_string()})
        )?;
    }
    trace.sync_all()?;
    result?;
    raw_result?;
    notepad_cleanup?;
    browser_cleanup?;
    if record {
        let output = run.join("workflow.json");
        std::fs::write(&output, serde_json::to_vec_pretty(&captured)?)?;
        println!(
            "录制完成，{} 个动作；workflow：{}",
            captured.steps.len(),
            output.display()
        );
    } else {
        println!(
            "回放完成，{} 个动作；{} 次逐条保存、{} 条本地发送气泡已确认",
            captured.steps.len(),
            captured
                .steps
                .iter()
                .filter(|s| matches!(s, Step::NotepadSave))
                .count(),
            captured
                .steps
                .iter()
                .filter(|s| matches!(s, Step::WechatSend { .. }))
                .count()
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn execute(
    workflow: &Workflow,
    captured: &mut Workflow,
    browser: &mut Browser,
    notepad: &mut Option<Notepad>,
    recording: &mut Option<RawRecording>,
    root: &Path,
    run: &Path,
    marker: &str,
    trace: &mut std::fs::File,
) -> Result<()> {
    let mut browser_text = None;
    let mut notepad_text = None;
    for (index, step) in workflow.steps.iter().enumerate() {
        println!("动作 {}：{step:?}", index + 1);
        writeln!(
            trace,
            "{}",
            serde_json::json!({"step":index,"status":"started","command":step})
        )?;
        trace.sync_data()?;
        let evidence = match step {
            Step::BrowserOpen { url } => match browser.request(Request::Open { url }).await? {
                Response::Opened { url } => serde_json::json!({"url":url}),
                other => return Err(format!("浏览器打开协议错误：{other:?}").into()),
            },
            Step::NotepadOpen => {
                if notepad.is_some() {
                    return Err("记事本重复打开".into());
                }
                let editor = Notepad::open(&run.join(format!("{marker}.txt"))).await?;
                *recording = Some(RawRecording::start(
                    &run.join("notepad-input.jsonl"),
                    editor.window.process_id(),
                )?);
                *notepad = Some(editor);
                serde_json::json!({"application":"Windows Notepad"})
            }
            Step::BrowserRead { selector, index } => match browser
                .request(Request::Read {
                    selector,
                    index: *index,
                })
                .await?
            {
                Response::Item { title, url } => {
                    browser_text = Some(title.clone());
                    serde_json::json!({"title":title,"url":url})
                }
                other => return Err(format!("浏览器读取协议错误：{other:?}").into()),
            },
            Step::NotepadAppend {
                query,
                prefix_run_id,
            } => {
                let title = browser_text.take().ok_or("没有可消费的浏览器条目")?;
                // 本轮编号用于区分录制和回放；发送前另行拒绝当前可见的同文消息。
                let text = if *prefix_run_id {
                    format!("{}{}", &marker[marker.len() - 6..], title)
                } else {
                    title
                };
                notepad
                    .as_mut()
                    .ok_or("记事本未打开")?
                    .append(&text, query)
                    .await?;
                serde_json::json!({"observed_text":text})
            }
            Step::NotepadSave => {
                notepad.as_ref().ok_or("记事本未打开")?.save().await?;
                serde_json::json!({"saved_file_matches_editor":true})
            }
            Step::NotepadRead { query, line } => {
                let text = notepad
                    .as_ref()
                    .ok_or("记事本未打开")?
                    .read_text(query)
                    .await?;
                let line_text = text
                    .lines()
                    .nth(*line)
                    .filter(|s| !s.is_empty())
                    .ok_or("记事本缺少指定行")?
                    .to_string();
                notepad_text = Some(line_text.clone());
                serde_json::json!({"uia_line":line_text,"line":line})
            }
            Step::WechatSend { .. } => {
                let text = notepad_text.take().ok_or("必须先从记事本读取一条消息")?;
                let editor = notepad.as_ref().ok_or("记事本未打开")?;
                let events =
                    wechat::send(&text, root, run, index, editor.input(), editor.runtime()).await?;
                serde_json::json!({"message":text,"outcome":"LocalBubbleObserved","raw_events":events})
            }
        };
        writeln!(
            trace,
            "{}",
            serde_json::json!({"step":index,"status":"completed","command":step,"evidence":evidence})
        )?;
        trace.sync_data()?;
        captured.steps.push(step.clone());
    }
    Ok(())
}

/// 不发送消息的原生前置检查。
pub async fn check_notepad(directory: &Path) -> Result<()> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let path = directory
        .join("output")
        .join(format!("notepad-check-{stamp}.txt"));
    std::fs::create_dir_all(path.parent().ok_or("缺少输出目录")?)?;
    let mut editor = Notepad::open(&path).await?;
    let result = async {
        editor
            .append("记事本逐条保存测试", super::notepad::EDITOR_QUERY)
            .await?;
        editor.save().await?;
        println!(
            "UIA 读回：{}",
            editor.read_text(super::notepad::EDITOR_QUERY).await?
        );
        Ok::<_, Box<dyn std::error::Error>>(())
    }
    .await;
    let cleanup = editor.shutdown().await;
    result?;
    cleanup
}
