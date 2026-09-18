//! 每次运行独占一个 JSONL 文件；逐条写入，不受界面日志条数限制。
use super::messages::{LogEntry, RunSnapshot};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

pub(super) struct RunLogFile {
    file: tokio::fs::File,
    path: PathBuf,
    failure: Option<String>,
}
impl RunLogFile {
    /// 在执行任何工作流动作前确认目录与文件可写，随机文件名避免重启后运行 ID 重复。
    pub async fn create(directory: &Path) -> Result<Self, String> {
        tokio::fs::create_dir_all(directory)
            .await
            .map_err(|e| format!("无法创建运行日志目录：{e}"))?;
        let path = directory.join(format!("run-{}.jsonl", uuid::Uuid::new_v4()));
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
            .map_err(|e| format!("无法创建运行日志 {}：{e}", path.display()))?;
        Ok(Self {
            file,
            path,
            failure: None,
        })
    }
    pub fn path(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
    async fn write(&mut self, value: serde_json::Value) {
        if self.failure.is_some() {
            return;
        }
        let result = async {
            let mut bytes = serde_json::to_vec(&value).map_err(std::io::Error::other)?;
            bytes.push(b'\n');
            self.file.write_all(&bytes).await?;
            // 不等待工作流结束才提交缓冲，异常退出仍保留已写入的事件。
            self.file.flush().await
        }
        .await;
        if let Err(error) = result {
            self.failure = Some(format!("运行日志写入失败 {}：{error}", self.path.display()));
        }
    }
    pub async fn begin(&mut self, snapshot: &RunSnapshot) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|time| time.as_millis().to_string())
            .unwrap_or_default();
        self.write(
            serde_json::json!({"type":"run_started", "run_id":snapshot.id,
            "workflow":snapshot.workflow, "started_unix_ms":timestamp}),
        )
        .await;
    }
    pub async fn append(&mut self, entry: &LogEntry) {
        self.write(serde_json::json!({"type":"event", "entry":entry}))
            .await;
    }
    pub async fn finish(&mut self, snapshot: &RunSnapshot, elapsed_ms: u128) {
        self.write(serde_json::json!({"type":"run_finished", "run_id":snapshot.id,
            "elapsed_ms":elapsed_ms.to_string(), "status":snapshot.status, "errors":snapshot.errors})).await;
        if self.failure.is_none()
            && let Err(error) = self.file.sync_all().await
        {
            self.failure = Some(format!("运行日志同步失败 {}：{error}", self.path.display()));
        }
    }
    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }
}

#[cfg(test)]
#[path = "../../../tests/argusflow-desktop/unit/log_file.rs"]
mod tests;
