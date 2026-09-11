//! 运行记录和订阅共用一把锁，确保快照始终先于后续增量消息。
use super::messages::{DesktopRunStatus, LogEntry, RunMessage, RunSnapshot, errors, locations};
use crate::document::wire::encode_values;
use argusflow_runtime::{RunResult, RunStatus};
use tauri::ipc::Channel;
use tokio::sync::Mutex;

/// 宿主和前端均只保留最近五千条日志。
pub(super) const LOG_LIMIT: usize = 5000;

#[cfg(test)]
#[path = "../../../tests/argusflow-desktop/unit/journal.rs"]
mod tests;

#[derive(Default)]
struct JournalState {
    snapshot: Option<RunSnapshot>,
    channel: Option<Channel<RunMessage>>,
}
impl JournalState {
    fn send(&mut self, message: RunMessage) {
        if let Some(channel) = &self.channel
            && channel.send(message).is_err()
        {
            // 页面退出只移除订阅；宿主继续持有运行记录和最终结果。
            self.channel = None;
        }
    }
}

/// 运行日志、完整结果及有序订阅的单一所有者。
#[derive(Default)]
pub(super) struct RunJournal(Mutex<JournalState>);
impl RunJournal {
    pub async fn begin(&self, snapshot: RunSnapshot, channel: Channel<RunMessage>) {
        let mut state = self.0.lock().await;
        state.snapshot = Some(snapshot.clone());
        state.channel = Some(channel);
        state.send(RunMessage::Snapshot { snapshot });
    }

    pub async fn append(&self, entry: LogEntry) {
        let mut state = self.0.lock().await;
        if let Some(snapshot) = &mut state.snapshot {
            push(snapshot, entry.clone());
            let id = snapshot.id.clone();
            state.send(RunMessage::Log { id, entry });
        }
    }

    pub async fn progress(&self, status: RunStatus) {
        if status == RunStatus::Running {
            return;
        }
        let mut state = self.0.lock().await;
        if let Some(snapshot) = &mut state.snapshot
            && snapshot.status == DesktopRunStatus::Running
        {
            // 引擎终态出现时，事件队列可能尚未消费完，工作台继续保持收尾状态。
            snapshot.status = DesktopRunStatus::Cleaning;
            let id = snapshot.id.clone();
            state.send(RunMessage::Status {
                id,
                status: DesktopRunStatus::Cleaning,
            });
        }
    }

    pub async fn finish(&self, result: &RunResult, elapsed_ms: u128) {
        let mut state = self.0.lock().await;
        if let Some(snapshot) = &mut state.snapshot {
            snapshot.status = result.status.into();
            match encode_values(&result.outputs) {
                Ok(outputs) => snapshot.outputs = outputs,
                Err(message) => {
                    snapshot.status = DesktopRunStatus::Failed;
                    snapshot.errors.push(message);
                }
            }
            if let Some(error) = &result.error {
                snapshot.errors.extend(errors(error));
            }
            if !snapshot.errors.is_empty() {
                let entry = LogEntry {
                    sequence: "final-error".into(),
                    elapsed_ms: elapsed_ms.to_string(),
                    kind: "error".into(),
                    level: "error".into(),
                    message: snapshot.errors.join("；"),
                    path: result
                        .error
                        .as_ref()
                        .map(|error| locations(&error.path))
                        .unwrap_or_default(),
                };
                push(snapshot, entry);
            }
            let snapshot = snapshot.clone();
            state.send(RunMessage::Snapshot { snapshot });
        }
    }

    pub async fn snapshot(&self) -> Option<RunSnapshot> {
        self.0.lock().await.snapshot.clone()
    }

    pub async fn subscribe(&self, channel: Channel<RunMessage>) {
        let mut state = self.0.lock().await;
        state.channel = Some(channel);
        if let Some(snapshot) = state.snapshot.clone() {
            state.send(RunMessage::Snapshot { snapshot });
        }
    }
}

fn push(snapshot: &mut RunSnapshot, entry: LogEntry) {
    if snapshot.logs.len() >= LOG_LIMIT {
        snapshot.logs.remove(0);
        snapshot.omitted += 1;
    }
    snapshot.logs.push(entry);
}
