//! 复用实际 Hook 监听，限定目标进程；不把注入输入伪装成人工输入。
use super::Result;
use argusflow_windows::listening::InputListener;
use std::{io::Write, path::Path, sync::mpsc, thread::JoinHandle};

pub struct RawRecording {
    listener: InputListener,
    writer: JoinHandle<std::result::Result<usize, String>>,
}
impl RawRecording {
    pub fn start(path: &Path, pid: u32) -> Result<Self> {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)?;
        let (sender, receiver) = mpsc::sync_channel(8192);
        let listener = InputListener::start_scoped(sender, std::process::id(), Some(pid))?;
        let state = listener.state();
        let writer = std::thread::spawn(move || {
            let mut count = 0;
            for event in receiver {
                state.consumed();
                serde_json::to_writer(&mut file, &event).map_err(|e| e.to_string())?;
                file.write_all(b"\n").map_err(|e| e.to_string())?;
                count += 1;
            }
            file.sync_all().map_err(|e| e.to_string())?;
            Ok(count)
        });
        Ok(Self { listener, writer })
    }
    pub fn finish(mut self) -> Result<usize> {
        self.listener.shutdown()?;
        let lost = self.listener.state().lost();
        let count = self.writer.join().map_err(|_| "录制写线程异常")??;
        if lost != 0 {
            return Err(format!("录制丢失 {lost} 条输入").into());
        }
        Ok(count)
    }
}
