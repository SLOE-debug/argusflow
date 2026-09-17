//! 父进程托管独立写者和有界证据任务，不持有原生 Hook。
use super::{evidence::EvidenceServices, messages::*, pipes};
use argusflow_recorder::*;
use std::os::windows::process::CommandExt;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{Mutex, mpsc, oneshot};

pub(super) struct Active {
    pub control: mpsc::Sender<(SessionPhase, oneshot::Sender<Result<(), String>>)>,
    pub task: tokio::task::JoinHandle<()>,
}
/// 跨 UI 生命周期的录制服务，关闭应用时先停止此服务。
#[derive(Default)]
pub struct RecorderManager {
    pub(super) active: Mutex<Option<Active>>,
    pub(super) status: Arc<Mutex<RecorderStatus>>,
}
impl RecorderManager {
    pub async fn status(&self) -> RecorderStatus {
        self.status.lock().await.clone()
    }
    pub async fn start(
        &self,
        root: PathBuf,
        runs: &crate::runtime::RunManager,
    ) -> Result<String, String> {
        let mut active = self.active.lock().await;
        if active.as_ref().is_some_and(|a| !a.task.is_finished()) {
            return Err("已有录制正在运行或收尾".into());
        }
        *active = None;
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let services = EvidenceServices::start(runs).await?;
        let id = uuid::Uuid::new_v4().to_string();
        let directory = root.join(&id);
        let origin = argusflow_windows::listening::qpc();
        let frequency = argusflow_windows::listening::qpc_frequency()?;
        let session=Session{id:id.clone(),format:2,created_ms:std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_millis() as u64,qpc_origin:origin,qpc_frequency:frequency,capture_session:None,policy:"video:DXGI/D3D11/MF hardware H264;native SDR;30fps maximum;8Mbps VBR;4GiB video budget;pause finalizes segment;QPC frame index;full display pixels".into()};
        let mut child =
            std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .arg("--recorder-child")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .creation_flags(0x08000000)
                .spawn()
                .map_err(|e| e.to_string())?;
        let mut stdin = child.stdin.take().ok_or("子进程缺少输入管道")?;
        let stdout = child.stdout.take().ok_or("子进程缺少输出管道")?;
        let (commands, mut outbound) = mpsc::channel::<RecorderCommand>(512);
        std::thread::spawn(move || {
            while let Some(command) = outbound.blocking_recv() {
                if pipes::write(&mut stdin, &command).is_err() {
                    break;
                }
            }
        });
        let (incoming, notices) = mpsc::channel(8192);
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stdout);
            loop {
                match pipes::read::<RecorderNotice>(&mut reader) {
                    Ok(Some(notice)) => {
                        if incoming.blocking_send(notice).is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let _ = incoming.blocking_send(RecorderNotice::Fault(error));
                        break;
                    }
                }
            }
        });
        commands
            .send(RecorderCommand::Start(StartRequest {
                directory: directory.to_string_lossy().into_owned(),
                session: session.clone(),
                excluded_pid: std::process::id(),
                included_pid: None,
            }))
            .await
            .map_err(|e| e.to_string())?;
        *self.status.lock().await = RecorderStatus {
            session: Some(id.clone()),
            ..Default::default()
        };
        let (control, controls) = mpsc::channel(4);
        let (ready_tx, ready_rx) = oneshot::channel();
        let state = self.status.clone();
        let task = tokio::spawn(super::supervisor::run(
            child, commands, notices, controls, state, services, directory, session, ready_tx,
        ));
        *active = Some(Active { control, task });
        drop(active);
        match tokio::time::timeout(Duration::from_secs(8), ready_rx).await {
            Ok(Ok(result)) => result?,
            _ => {
                self.force_stop().await;
                return Err("录制子进程未在8秒内就绪".into());
            }
        }
        Ok(id)
    }
    pub async fn transition(&self, phase: SessionPhase) -> Result<(), String> {
        let active = self.active.lock().await;
        let Some(active) = active.as_ref().filter(|a| !a.task.is_finished()) else {
            return Err("没有活动录制".into());
        };
        let (tx, rx) = oneshot::channel();
        active
            .control
            .try_send((phase, tx))
            .map_err(|_| "录制控制正在处理，请稍后重试".to_string())?;
        tokio::time::timeout(Duration::from_secs(12), rx)
            .await
            .map_err(|_| "录制控制收尾超时".to_string())?
            .map_err(|_| "录制进程已退出".to_string())?
    }
    async fn force_stop(&self) {
        if let Some(active) = self.active.lock().await.take() {
            active.task.abort();
        }
        let mut state = self.status.lock().await;
        state.phase = Some(SessionPhase::Faulted);
        state.input_fault = Some("子进程握手失败，已请求回收".into());
    }
    pub async fn shutdown(&self) -> Result<(), String> {
        if self
            .active
            .lock()
            .await
            .as_ref()
            .is_some_and(|a| !a.task.is_finished())
        {
            self.transition(SessionPhase::Stopped).await?;
        }
        Ok(())
    }
}
