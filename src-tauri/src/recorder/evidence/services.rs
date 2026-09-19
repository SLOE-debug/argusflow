//! 录制结构观察借用共享 UIA，画面由独立视频链路负责。
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[derive(Clone)]
pub(crate) struct EvidenceServices {
    pub(super) uia: Option<argusflow_windows::UiaRuntime>,
    pub(super) observing: Arc<AtomicBool>,
    pub(super) clipboard: argusflow_windows::ClipboardReader,
    pub(super) clipboard_sequence: Arc<tokio::sync::Mutex<Option<u32>>>,
    pub(super) previous: Arc<tokio::sync::Mutex<Option<super::structure::PreviousObservation>>>,
}
impl EvidenceServices {
    pub async fn metadata(
        &self,
        id: u64,
        event: argusflow_input_contracts::InputEvent,
        tx: &tokio::sync::mpsc::Sender<argusflow_recorder::RecorderCommand>,
    ) {
        let observed = self.structure(&[id], event, tx).await;
        // 只有明确取得非敏感焦点才读取正文；未知或敏感期间重置基线。
        if observed.as_ref().is_some_and(|s| !s.sensitive && !s.stale) {
            self.observe_clipboard(id, tx).await;
        } else {
            *self.clipboard_sequence.lock().await = None;
        }
        let _ = tx
            .send(argusflow_recorder::RecorderCommand::evidence(
                argusflow_recorder::RecordData::Attempt {
                    raw: vec![id],
                    stage: argusflow_recorder::Stage::Derivation,
                    outcome: argusflow_recorder::Outcome::Complete,
                    reason: "结构观察结束；画面按QPC从独立视频读取，不代表此帧已持久化".into(),
                },
            ))
            .await;
    }

    pub async fn start(runs: &crate::runtime::RunManager) -> Result<Self, String> {
        Ok(Self {
            uia: runs.recording_uia().await?,
            observing: Arc::new(AtomicBool::new(true)),
            clipboard: argusflow_windows::ClipboardReader::start().map_err(|e| e.to_string())?,
            clipboard_sequence: Arc::new(tokio::sync::Mutex::new(None)),
            previous: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }
    pub async fn pause(&self) -> Result<(), String> {
        self.observing.store(false, Ordering::Release);
        Ok(())
    }
    pub async fn resume(&self) -> Result<(), String> {
        *self.clipboard_sequence.lock().await = None;
        *self.previous.lock().await = None;
        self.observing.store(true, Ordering::Release);
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<(), String> {
        self.observing.store(false, Ordering::Release);
        self.clipboard
            .shutdown(super::options(500))
            .await
            .map_err(|e| e.to_string())
    }
}
