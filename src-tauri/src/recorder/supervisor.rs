//! 输入写者与视频子进程的共同边界；就绪、暂停、恢复、最终同步明确分开。
use super::{evidence::EvidenceServices, messages::RecorderStatus, video::VideoProcess};
use argusflow_recorder::*;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, mpsc, oneshot};
struct ChildGuard(std::process::Child);
struct HeartbeatGuard(tokio::task::JoinHandle<()>);
impl Drop for HeartbeatGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
type Boundary = (SessionPhase, oneshot::Sender<Result<(), String>>, Instant);

#[allow(clippy::too_many_arguments)]
pub(super) async fn run(
    child: std::process::Child,
    commands: mpsc::Sender<RecorderCommand>,
    mut notices: mpsc::Receiver<RecorderNotice>,
    mut controls: mpsc::Receiver<(SessionPhase, oneshot::Sender<Result<(), String>>)>,
    state: Arc<Mutex<RecorderStatus>>,
    services: EvidenceServices,
    directory: PathBuf,
    session: Session,
    ready: oneshot::Sender<Result<(), String>>,
) {
    let _child = ChildGuard(child);
    let mut ready = Some(ready);
    let mut boundary: Option<Boundary> = None;
    let mut video: Option<VideoProcess> = None;
    // 启动/收尾原生进程期间也持续发送输入写者心跳。
    let pulse = commands.clone();
    let heartbeat = HeartbeatGuard(tokio::spawn(async move {
        loop {
            if pulse.send(RecorderCommand::Heartbeat).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }));
    let result:Result<(),String>=async {
        let mut phase=SessionPhase::Paused;
        let mut jobs=tokio::task::JoinSet::new();
        let mut interval=tokio::time::interval(Duration::from_millis(100));
        let started=Instant::now();
        loop {
            tokio::select! {
                notice=notices.recv()=>match notice {
                    Some(RecorderNotice::Ready)=>{
                        video=Some(VideoProcess::start(&directory).await?);
                        commands.send(RecorderCommand::Transition(SessionPhase::Recording)).await.map_err(|e|e.to_string())?;
                    },
                    Some(RecorderNotice::InputSuspended)=>{
                        services.pause().await?;
                        jobs.abort_all();
                        while jobs.join_next().await.is_some() {}
                        if let Some(process)=video.take(){process.stop().await?;}
                        let target=boundary.as_ref().ok_or("缺少暂停边界")?.0;
                        let next=if target==SessionPhase::Stopped {SessionPhase::Stopping}else{target};
                        commands.send(RecorderCommand::Transition(next)).await.map_err(|e|e.to_string())?;
                    },
                    Some(RecorderNotice::Written(record))=>{
                        let mut status=state.lock().await;
                        status.written=record.id;
                        match record.data {
                            RecordData::Raw(event)=>{
                                status.raw_count+=1;
                                if status.log_latency_us.len()<10000 {
                                    status.log_latency_us.push(((argusflow_windows::listening::qpc()-event.qpc).max(0) as u128*1_000_000/u128::from(session.qpc_frequency)) as u64);
                                }
                                if requires_evidence(event) && jobs.len()<2 {
                                    let services=services.clone();let tx=commands.clone();
                                    jobs.spawn(async move {services.metadata(record.id,event,&tx).await;});
                                } else if requires_evidence(event) {
                                    commands.try_send(RecorderCommand::evidence(RecordData::Attempt{raw:vec![record.id],stage:Stage::Derivation,outcome:Outcome::Unavailable,reason:"结构观察并发已满；输入和视频独立保存，可按时间回看".into()})).map_err(|_|"记录结构观察缺口失败")?;
                                }
                            },
                            RecordData::Interaction(action)=>{if action.kind!=InteractionKind::DoubleClick{status.operations+=1;}},
                            RecordData::Attempt{outcome,..}=>{if outcome.is_evidence_gap(){status.evidence_gaps+=1;}},
                            RecordData::State{phase:observed,..}=>{
                                phase=observed;status.phase=Some(if observed==SessionPhase::Stopped {SessionPhase::Stopping}else{observed});
                                if observed==SessionPhase::Stopping {commands.send(RecorderCommand::Transition(SessionPhase::Stopped)).await.map_err(|e|e.to_string())?;}
                                if observed==SessionPhase::Recording && let Some(reply)=ready.take(){let _=reply.send(Ok(()));}
                                if observed!=SessionPhase::Stopped && boundary.as_ref().is_some_and(|b|b.0==observed) {
                                    let (_,reply,_)=boundary.take().ok_or("缺少边界")?;let _=reply.send(Ok(()));
                                }
                            },_=>{}
                        }
                    },
                    Some(RecorderNotice::Synced(mark))=>state.lock().await.synced=mark,
                    Some(RecorderNotice::Fault(error))=>return Err(error),
                    Some(RecorderNotice::Finished)=>{
                        if boundary.as_ref().is_none_or(|b|b.0!=SessionPhase::Stopped){return Err("输入写者意外退出".into());}
                        state.lock().await.phase=Some(SessionPhase::Stopped);
                        let (_,reply,_)=boundary.take().ok_or("缺少停止边界")?;let _=reply.send(Ok(()));break;
                    },
                    None=>return Err("输入写者连接中断".into()),
                },
                Some((target,reply))=controls.recv()=>{
                    if boundary.is_some(){let _=reply.send(Err("已有状态转换正在处理".into()));continue;}
                    if !matches!((phase,target),(SessionPhase::Recording,SessionPhase::Paused|SessionPhase::Stopped)|(SessionPhase::Paused,SessionPhase::Recording|SessionPhase::Stopped)) {
                        let _=reply.send(Err("当前状态不能执行此操作".into()));continue;
                    }
                    boundary=Some((target,reply,Instant::now()));
                    if target==SessionPhase::Recording {
                        video=Some(VideoProcess::start(&directory).await?);
                        services.resume().await?;
                        commands.send(RecorderCommand::Transition(target)).await.map_err(|e|e.to_string())?;
                    } else if phase==SessionPhase::Recording {
                        commands.send(RecorderCommand::SuspendInput).await.map_err(|e|e.to_string())?;
                    } else {
                        commands.send(RecorderCommand::Transition(SessionPhase::Stopping)).await.map_err(|e|e.to_string())?;
                    }
                },
                Some(job)=jobs.join_next()=>{job.map_err(|e|e.to_string())?;},
                _=interval.tick()=>{
                    if let Some(process)=&mut video {process.check()?;}
                    let mut status=state.lock().await;
                    status.pending=jobs.len();status.elapsed_ms=started.elapsed().as_millis() as u64;
                    if boundary.as_ref().is_some_and(|b|b.2.elapsed()>Duration::from_secs(10)){return Err("录制状态收尾超时".into());}
                }
            }
        }
        Ok(())
    }.await;
    drop(heartbeat);
    drop(video);
    let _ = services.pause().await;
    let result = result.and(services.shutdown().await);
    let mut status = state.lock().await;
    status.pending = 0;
    if let Err(error) = result {
        status.phase = Some(SessionPhase::Faulted);
        status.input_fault = Some(error.clone());
        if let Some(reply) = ready.take() {
            let _ = reply.send(Err(error.clone()));
        }
        if let Some((_, reply, _)) = boundary.take() {
            let _ = reply.send(Err(error));
        }
    }
}
