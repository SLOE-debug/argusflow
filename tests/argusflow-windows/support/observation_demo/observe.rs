//! 不接收示范计划，只接收系统事件和独立采样；不执行用户动作。
use super::{Result, samples::Sampler};
use argusflow_recorder::Normalizer;
use argusflow_windows::listening::{InputListener, gesture_settings, qpc, qpc_frequency};
use std::{
    io::Write,
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};
pub fn write(file: &mut std::fs::File, value: &impl serde::Serialize) -> Result<()> {
    serde_json::to_writer(&mut *file, value)?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}
pub async fn run(directory: &Path, seconds: u64, root: &Path) -> Result<()> {
    if !(1..=600).contains(&seconds) {
        return Err("录制时长应为1–600秒".into());
    }
    std::fs::create_dir_all(directory)?;
    let mut events = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("events.jsonl"))?;
    let mut interactions = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("interactions.jsonl"))?;
    let mut sampler = Sampler::start(directory, root).await?;
    let (tx, rx) = mpsc::sync_channel(8192);
    let mut listener = InputListener::start(tx, std::process::id())?;
    let state = listener.state();
    let frequency = qpc_frequency()?;
    let (double_ms, distance) = gesture_settings();
    let mut normalizer = Normalizer::new(frequency, double_ms, distance);
    std::fs::write(
        directory.join("session.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"qpc":qpc(),"epoch_ms":super::samples::epoch_ms(),"frequency":frequency,"seconds":seconds,"mode":"passive; no action plan input"}),
        )?,
    )?;
    println!("OBSERVING {}", directory.display());
    let started = Instant::now();
    let outcome = async {
        while started.elapsed() < Duration::from_secs(seconds) && !directory.join("stop").exists() {
            while let Ok(event) = rx.try_recv() {
                state.consumed();
                write(&mut events, &event)?;
                if let Some(action) = normalizer.push(event.sequence, event) {
                    write(&mut interactions, &action)?;
                }
            }
            sampler.sample().await?;
            if state.lost() != 0 {
                return Err::<(), Box<dyn std::error::Error>>(
                    "原始事件丢失，不能生成完整 workflow".into(),
                );
            }
            tokio::time::sleep(Duration::from_millis(600)).await;
        }
        Ok(())
    }
    .await;
    listener.shutdown()?;
    while let Ok(event) = rx.try_recv() {
        state.consumed();
        write(&mut events, &event)?;
        if let Some(action) = normalizer.push(event.sequence, event) {
            write(&mut interactions, &action)?;
        }
    }
    let cleanup = sampler.shutdown().await;
    events.sync_all()?;
    interactions.sync_all()?;
    std::fs::write(
        directory.join("finished.json"),
        serde_json::to_vec(
            &serde_json::json!({"lost":state.lost(),"complete":outcome.is_ok(),"error":outcome.as_ref().err().map(ToString::to_string)}),
        )?,
    )?;
    outcome?;
    cleanup?;
    Ok(())
}
