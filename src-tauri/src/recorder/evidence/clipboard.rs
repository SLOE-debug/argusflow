//! 连接平台剪贴板观察与已提交输入；不根据 Ctrl+C 猜测内容。
use super::{attempt, options, services::EvidenceServices};
use argusflow_recorder::{Clipboard, Outcome, RecordData, RecorderCommand, Stage};
use argusflow_windows::listening::qpc;
use std::sync::atomic::Ordering;
impl EvidenceServices {
    pub(super) async fn observe_clipboard(
        &self,
        id: u64,
        tx: &tokio::sync::mpsc::Sender<RecorderCommand>,
    ) {
        let mut previous = self.clipboard_sequence.lock().await;
        if !self.observing.load(Ordering::Acquire) {
            return;
        }
        let from_qpc = qpc();
        let result = self.clipboard.observe(*previous, options(250)).await;
        if !self.observing.load(Ordering::Acquire) {
            return;
        }
        let data = match result {
            Ok(observation) => {
                *previous = Some(observation.sequence);
                RecordData::Clipboard(Clipboard {
                    raw: vec![id],
                    from_qpc,
                    through_qpc: qpc(),
                    observation,
                })
            }
            Err(error) => attempt(
                &[id],
                Stage::Clipboard,
                Outcome::Unavailable,
                &error.to_string(),
            ),
        };
        let _ = tx.send(RecorderCommand::evidence(data)).await;
    }
}
