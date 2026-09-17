//! 录制结构观察借用共享 UIA，画面由独立视频链路负责。
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[derive(Clone)]
pub(crate) struct EvidenceServices {
    pub(super) uia: Option<argusflow_windows::UiaRuntime>,
    pub(super) observing: Arc<AtomicBool>,
}
impl EvidenceServices {
    pub async fn metadata(
        &self,
        id: u64,
        event: argusflow_input_contracts::InputEvent,
        tx: &tokio::sync::mpsc::Sender<argusflow_recorder::RecorderCommand>,
    ) {
        self.structure(&[id], event, tx).await;
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
        })
    }
    pub async fn pause(&self) -> Result<(), String> {
        self.observing.store(false, Ordering::Release);
        Ok(())
    }
    pub async fn resume(&self) -> Result<(), String> {
        self.observing.store(true, Ordering::Release);
        Ok(())
    }
    pub async fn shutdown(&self) {
        self.observing.store(false, Ordering::Release);
    }
}
