//! 录制生命周期与后台任务编排；证据采集和文件编码由独立模块负责。

use crate::{
    EvidenceCollector, RecorderError, RecorderPhase, RecorderStatus, RecordingFiles,
    RecordingSummary, RecordingTrace, hooks::HookCapture,
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use windows::Win32::System::SystemInformation::GetTickCount;

/// 停止后的文件与结构化结果。
#[derive(Debug, Clone, Serialize)]
pub struct CompletedRecording {
    /// 时间线、manifest 与图像证据目录。
    pub files: RecordingFiles,
    /// 供当前调用方立即使用的完整 Trace。
    pub trace: RecordingTrace,
}

/// 应用级单录制控制器；必须显式 start，构造不会安装全局 Hook。
pub struct RecorderService {
    /// 与执行器共用能力实例的反查门面。
    resolver: Arc<EvidenceCollector>,
    /// 默认位于宿主 .argusflow/recordings。
    root: PathBuf,
    /// start/stop 的互斥生命周期边界。
    session: Mutex<Option<ActiveRecording>>,
}

/// Hook、ingestion 与 async worker 均由同一个录制生命周期拥有。
struct ActiveRecording {
    /// 控制面识别本次会话。
    id: uuid::Uuid,
    /// 专用消息线程的停止句柄。
    hook: HookCapture,
    /// 元数据摄入线程；hook 关闭后输入 channel 自然结束。
    ingestion: Option<std::thread::JoinHandle<()>>,
    /// 按序脱敏并关联证据的异步结果。
    worker: tokio::task::JoinHandle<RecordingTrace>,
    /// 文件写入失败或调用取消时仍保留可重试的 Trace。
    completed: Option<RecordingTrace>,
    /// 输入丢弃的唯一累计计数。
    dropped: Arc<AtomicU64>,
    /// Worker 仅在完成脱敏后发布计数。
    processed: Arc<AtomicU64>,
    /// UI 的计时起点。
    started_at_unix_ms: u64,
}

impl RecorderService {
    /// 构造只保存配置的控制器，不访问用户输入。
    pub fn new(resolver: Arc<EvidenceCollector>, root: impl Into<PathBuf>) -> Self {
        Self {
            resolver,
            root: root.into(),
            session: Mutex::new(None),
        }
    }

    /// 安装真实输入 Hook 并启动两个 worker 阶段；拒绝重复录制。
    pub async fn start(&self) -> Result<RecorderStatus, RecorderError> {
        let mut session = self.session.lock().await;
        if session.is_some() {
            return Err(RecorderError::AlreadyRecording);
        }
        let id = uuid::Uuid::new_v4();
        // 先确认演示包可写，再安装 Hook；截图在录制期间立即写入此目录。
        let directory = self.root.join(id.to_string());
        tokio::fs::create_dir_all(directory.join("evidence")).await?;
        let screenshots = crate::screenshot_pipeline::ScreenshotWriter::start(directory)?;
        let dropped = Arc::new(AtomicU64::new(0));
        let processed = Arc::new(AtomicU64::new(0));
        let (hook_sender, hook_receiver) = mpsc::sync_channel(4096);
        let (sender, receiver) = tokio::sync::mpsc::channel(2048);
        let resolver = self.resolver.clone();
        let ingestion_dropped = dropped.clone();
        // 与 Win32 tick 起点相邻采样，不能在安装 Hook 后再设置 Unix 时间原点。
        let started_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        // SAFETY: GetTickCount 与 Hook struct.time 使用相同事件时钟。
        let started_tick = unsafe { GetTickCount() };
        let ingestion = std::thread::Builder::new()
            .name("argusflow-recorder-ingest".into())
            .spawn(move || {
                crate::ingestion::ingest(
                    hook_receiver,
                    sender,
                    resolver,
                    ingestion_dropped,
                    started_tick,
                    screenshots,
                )
            })
            .map_err(|_| RecorderError::WorkerUnavailable)?;
        let hook = match HookCapture::start(hook_sender, dropped.clone()) {
            Ok(hook) => hook,
            Err(error) => {
                let _ = ingestion.join();
                return Err(error);
            }
        };
        let worker = tokio::spawn(crate::worker::record(
            receiver,
            self.resolver.clone(),
            dropped.clone(),
            id,
            started_at_unix_ms,
            processed.clone(),
        ));
        *session = Some(ActiveRecording {
            id,
            hook,
            ingestion: Some(ingestion),
            worker,
            completed: None,
            dropped,
            processed,
            started_at_unix_ms,
        });
        Ok(RecorderStatus {
            phase: RecorderPhase::Recording,
            recording_id: Some(id),
            started_at_unix_ms: Some(started_at_unix_ms),
            processed_events: 0,
            dropped_events: 0,
        })
    }

    /// 先卸载监听并排空事件及 PNG，再发布时间线；发布失败可再次 stop 重试。
    pub async fn stop(&self) -> Result<CompletedRecording, RecorderError> {
        let mut session = self.session.lock().await;
        let active = session.as_mut().ok_or(RecorderError::NotRecording)?;
        active.hook.stop()?;
        if let Some(ingestion) = active.ingestion.take() {
            tokio::task::spawn_blocking(move || ingestion.join())
                .await
                .map_err(|_| RecorderError::WorkerUnavailable)?
                .map_err(|_| RecorderError::WorkerUnavailable)?;
        }
        if active.completed.is_none() {
            match (&mut active.worker).await {
                Ok(trace) => active.completed = Some(trace),
                Err(_) => {
                    // 已完成的失败 JoinHandle 不能再次 poll；释放 session，允许用户重新开始。
                    *session = None;
                    return Err(RecorderError::WorkerUnavailable);
                }
            }
        }
        let trace = active
            .completed
            .as_ref()
            .ok_or(RecorderError::WorkerUnavailable)?;
        let files = crate::storage::save(&self.root, trace).await?;
        // 保存成功后移动结果，避免为大型 Raw Trace 再复制整份输入事实。
        let trace = session
            .take()
            .and_then(|active| active.completed)
            .ok_or(RecorderError::WorkerUnavailable)?;
        Ok(CompletedRecording { files, trace })
    }

    /// 返回只读快照；可用于展示全局录制是否开启和是否丢失事件。
    pub async fn status(&self) -> RecorderStatus {
        let session = self.session.lock().await;
        RecorderStatus {
            phase: session.as_ref().map_or(RecorderPhase::Idle, |active| {
                if active.hook.is_running() {
                    RecorderPhase::Recording
                } else if active.completed.is_some() {
                    RecorderPhase::AwaitingSave
                } else {
                    RecorderPhase::Finishing
                }
            }),
            recording_id: session.as_ref().map(|active| active.id),
            started_at_unix_ms: session.as_ref().map(|active| active.started_at_unix_ms),
            processed_events: session
                .as_ref()
                .map_or(0, |active| active.processed.load(Ordering::Relaxed)),
            dropped_events: session
                .as_ref()
                .map_or(0, |active| active.dropped.load(Ordering::Relaxed)),
        }
    }

    /// 最近最多 100 次完整保存的录制；未发布 manifest 的半成品不进入列表。
    pub async fn list(&self) -> Result<Vec<RecordingSummary>, RecorderError> {
        crate::history::list(&self.root).await
    }

    /// 通过 UUID 读取录制，调用方不能传入文件路径。
    pub async fn load(&self, id: uuid::Uuid) -> Result<CompletedRecording, RecorderError> {
        crate::history::load(&self.root, id).await
    }

    /// 只读取已发布事件的完整窗口图像或点击 crop。
    pub async fn screenshot(
        &self,
        id: uuid::Uuid,
        sequence: u64,
        kind: crate::ScreenshotKind,
    ) -> Result<Vec<u8>, RecorderError> {
        crate::evidence_reader::read(&self.root, id, sequence, kind).await
    }
}
