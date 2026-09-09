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
    pub(crate) root: PathBuf,
    /// start/stop 的互斥生命周期边界。
    pub(crate) session: Arc<Mutex<Option<ActiveRecording>>>,
}

/// Hook、ingestion 与 async worker 均由同一个录制生命周期拥有。
pub(crate) struct ActiveRecording {
    /// 精确索引已发布后待删除的候选像素，保存重试不得遗失清理任务。
    obsolete: Vec<PathBuf>,
    screen: Option<crate::screen_recording::ScreenRecording>,
    /// 控制面识别本次会话。
    pub(crate) id: uuid::Uuid,
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
    /// 应用退出时先完成活动归档，再关闭共享桌面采集主机。
    pub async fn shutdown(&self) -> Result<(), RecorderError> {
        let active = self.session.lock().await.is_some();
        let saved = if active {
            self.stop().await.map(|_| ())
        } else {
            Ok(())
        };
        self.resolver
            .shutdown_capture()
            .map_err(|_| RecorderError::WorkerUnavailable)?;
        saved
    }
    /// 构造只保存配置的控制器，不访问用户输入。
    pub fn new(resolver: Arc<EvidenceCollector>, root: impl Into<PathBuf>) -> Self {
        Self {
            resolver,
            root: root.into(),
            session: Arc::new(Mutex::new(None)),
        }
    }

    /// 安装真实输入 Hook 并启动两个 worker 阶段；拒绝重复录制。
    pub async fn start(&self) -> Result<RecorderStatus, RecorderError> {
        self.start_with_privacy(crate::RecordingPrivacy::default())
            .await
    }

    /// 会话隐私配置在开始后不可修改。
    pub async fn start_with_privacy(
        &self,
        privacy: crate::RecordingPrivacy,
    ) -> Result<RecorderStatus, RecorderError> {
        let mut session = self.session.lock().await;
        if session.is_some() {
            return Err(RecorderError::AlreadyRecording);
        }
        let id = uuid::Uuid::new_v4();
        // 先确认演示包可写，再安装 Hook；截图在录制期间立即写入此目录。
        let directory = self.root.join(id.to_string());
        tokio::fs::create_dir_all(directory.join("evidence")).await?;
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
        let origin_us = self
            .resolver
            .screen_clock_us()
            .map_err(|_| RecorderError::WorkerUnavailable)?;
        let screen = if privacy.screenshots() {
            None
        } else {
            Some(crate::screen_recording::ScreenRecording::start(
                self.resolver.clone(),
                directory.join("evidence"),
                origin_us,
            )?)
        };
        let ingestion = std::thread::Builder::new()
            .name("argusflow-recorder-ingest".into())
            .spawn(move || {
                crate::ingestion::ingest(
                    hook_receiver,
                    sender,
                    resolver,
                    ingestion_dropped,
                    started_tick,
                    privacy,
                )
            })
            .map_err(|_| RecorderError::WorkerUnavailable)?;
        let hook = match HookCapture::start_with_wake(
            hook_sender,
            dropped.clone(),
            screen.as_ref().map(|screen| screen.wake()),
        ) {
            Ok(hook) => hook,
            Err(error) => {
                let _ = ingestion.join();
                return Err(error);
            }
        };
        let worker = tokio::spawn(crate::worker::record_with_privacy(
            receiver,
            self.resolver.clone(),
            dropped.clone(),
            id,
            started_at_unix_ms,
            processed.clone(),
            privacy,
        ));
        let failure = screen.as_ref().map(|screen| screen.result());
        *session = Some(ActiveRecording {
            obsolete: Vec::new(),
            screen,
            id,
            hook,
            ingestion: Some(ingestion),
            worker,
            completed: None,
            dropped,
            processed,
            started_at_unix_ms,
        });
        if let Some(failure) = failure {
            let weak = Arc::downgrade(&self.session);
            let root = self.root.clone();
            let collector = self.resolver.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    let Some(session) = weak.upgrade() else {
                        break;
                    };
                    let mut session = session.lock().await;
                    let Some(active) = session
                        .as_mut()
                        .filter(|active| active.id == id && active.completed.is_none())
                    else {
                        break;
                    };
                    if failure.failed() {
                        // 归档失败立即卸载 Hook 并保存有效前缀，不触碰 Workflow 捕获服务。
                        if Self::finish_active(active).await.is_ok() {
                            if let Some(trace) = &mut active.completed {
                                let _ = crate::screen_finalization::finalize(
                                    &root,
                                    collector.clone(),
                                    trace,
                                    &mut active.obsolete,
                                )
                                .await;
                            }
                        }
                        break;
                    }
                }
            });
        }
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
        Self::finish_active(active).await?;
        let trace = active
            .completed
            .as_mut()
            .ok_or(RecorderError::WorkerUnavailable)?;
        let files = crate::screen_finalization::finalize(
            &self.root,
            self.resolver.clone(),
            trace,
            &mut active.obsolete,
        )
        .await?;
        let trace = session
            .take()
            .and_then(|active| active.completed)
            .ok_or(RecorderError::WorkerUnavailable)?;
        Ok(CompletedRecording { files, trace })
    }

    async fn finish_active(active: &mut ActiveRecording) -> Result<(), RecorderError> {
        active.hook.stop()?;
        if let Some(ingestion) = active.ingestion.take() {
            tokio::task::spawn_blocking(move || ingestion.join())
                .await
                .map_err(|_| RecorderError::WorkerUnavailable)?
                .map_err(|_| RecorderError::WorkerUnavailable)?;
        }
        if active.completed.is_none() {
            // 先冻结屏幕截止点，OCR/UIA worker 排空不能延长录制视觉区间。
            let screen = if let Some(mut screen) = active.screen.take() {
                tokio::task::spawn_blocking(move || screen.finish())
                    .await
                    .map_err(|_| RecorderError::WorkerUnavailable)?
            } else {
                crate::ScreenTimeline {
                    completeness: crate::ScreenCompleteness::Disabled,
                    ..Default::default()
                }
            };
            match (&mut active.worker).await {
                Ok(mut trace) => {
                    trace.screen = screen;
                    crate::screen_association::associate(&mut trace);
                    active.completed = Some(trace);
                }
                Err(_) => {
                    // 已完成的失败 JoinHandle 不能再次 poll；释放 session，允许用户重新开始。
                    return Err(RecorderError::WorkerUnavailable);
                }
            }
        }
        Ok(())
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
        let _session = self.session.lock().await;
        crate::history::list(&self.root).await
    }

    /// 通过 UUID 读取录制，调用方不能传入文件路径。
    pub async fn load(&self, id: uuid::Uuid) -> Result<CompletedRecording, RecorderError> {
        let _session = self.session.lock().await;
        crate::history::load(&self.root, id).await
    }

    /// 只读取已发布事件的完整窗口图像或点击 crop。
    pub async fn screenshot(
        &self,
        id: uuid::Uuid,
        sequence: u64,
        kind: crate::ScreenshotKind,
    ) -> Result<Vec<u8>, RecorderError> {
        let _session = self.session.lock().await;
        crate::evidence_reader::read(&self.root, id, sequence, kind).await
    }
}
