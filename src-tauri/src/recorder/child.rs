//! 子进程只拥有 Hook、输入追加日志和管道；不加载 UIA/DXGI/OCR。
use super::pipes;
use argusflow_recorder::*;
use argusflow_windows::listening::{InputListener, gesture_settings, qpc};
use std::{
    io::BufReader,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

/// 同一已打包可执行文件的专用子进程模式，不创建 Tauri 窗口。
pub fn run_child() -> Result<(), String> {
    let mut input = BufReader::new(std::io::stdin());
    let request = match pipes::read(&mut input)? {
        Some(RecorderCommand::Start(request)) => request,
        _ => return Err("缺少启动握手".into()),
    };
    let (double_ms, distance) = gesture_settings();
    let mut writer = RecordingWriter::create(
        std::path::Path::new(&request.directory),
        &request.session,
        double_ms,
        distance,
    )
    .map_err(|e| e.to_string())?;
    let (raw_tx, raw_rx) = mpsc::sync_channel(8192);
    let mut listener =
        InputListener::start_scoped(raw_tx, request.excluded_pid, request.included_pid)?;
    let state = listener.state();
    state.wait_paused()?;
    while raw_rx.try_recv().is_ok() {
        state.consumed();
    }
    let pipe_failed = Arc::new(AtomicBool::new(false));
    let (notice_tx, notice_rx) = mpsc::sync_channel(8192);
    let failure = pipe_failed.clone();
    let output = std::thread::spawn(move || {
        let mut output = std::io::stdout();
        while let Ok(notice) = notice_rx.recv() {
            if pipes::write(&mut output, &notice).is_err() {
                failure.store(true, Ordering::Release);
                break;
            }
        }
    });
    let (command_tx, command_rx) = mpsc::sync_channel(256);
    let parent_alive = Arc::new(AtomicBool::new(true));
    let _watchdog = super::watchdog::Watchdog::start(
        state.clone(),
        request.session.qpc_frequency,
        parent_alive.clone(),
    );
    let seen = _watchdog.heartbeat();
    let alive = parent_alive.clone();
    let paused = state.clone();
    std::thread::spawn(move || {
        while let Ok(Some(command)) = pipes::read::<RecorderCommand>(&mut input) {
            seen.store(qpc(), Ordering::Release);
            // 不等待日志磁盘同步才暂停 Hook。
            if matches!(
                command,
                RecorderCommand::SuspendInput
                    | RecorderCommand::Transition(
                        SessionPhase::Paused | SessionPhase::Stopping | SessionPhase::Stopped
                    )
            ) && paused.wait_paused().is_err()
            {
                break;
            }
            if command_tx.try_send(command).is_err() {
                break;
            }
        }
        paused.pause(true);
        alive.store(false, Ordering::Release);
    });
    let notify = |notice| {
        notice_tx
            .try_send(notice)
            .map_err(|_| "通知队列已满或父进程管道断开".to_string())
    };
    notify(RecorderNotice::written(
        writer
            .transition(
                SessionPhase::Paused,
                "输入已就绪，等待视频首帧后开始记录".into(),
                qpc(),
            )
            .map_err(|e| e.to_string())?,
    ))?;
    notify(RecorderNotice::Ready)?;
    let mut synced_at = Instant::now();
    let mut heartbeat = Instant::now();
    let mut phase = SessionPhase::Paused;
    let result = (|| -> Result<(), String> {
        loop {
            if state.finished() {
                return Err("输入监听线程意外退出".into());
            }
            if state.lost() > 0 {
                return Err(format!(
                    "原始输入队列溢出；已知丢失{}，峰值{}",
                    state.lost(),
                    state.peak()
                ));
            }
            if pipe_failed.load(Ordering::Acquire) {
                return Err("父进程输出管道断开".into());
            }
            if !parent_alive.load(Ordering::Acquire) || heartbeat.elapsed() > Duration::from_secs(3)
            {
                return Err("父进程失联；未提交的队列尾部不保证恢复".into());
            }
            // 队列总容量限制批次；暂停先关闭回调，再排空已接收事实。
            for _ in 0..8192 {
                let Ok(event) = raw_rx.try_recv() else {
                    break;
                };
                state.consumed();
                let record = writer.raw(event, qpc()).map_err(|e| e.to_string())?;
                // 先通知锚定，不等待 sync_data 或归一化。
                notify(RecorderNotice::written(record.clone()))?;
                if let Some(action) = writer
                    .normalize(&record, qpc())
                    .map_err(|e| e.to_string())?
                {
                    notify(RecorderNotice::written(action))?;
                }
            }
            // 暂停/停止命令必须位于已捕获输入之后，防止丢弃队列里的已接收事实。
            while let Ok(command) = command_rx.try_recv() {
                match command {
                    RecorderCommand::Heartbeat => heartbeat = Instant::now(),
                    RecorderCommand::SuspendInput => {
                        // 管道读线程先停 Hook；两次循环之间到达的尾部也必须先通知。
                        while let Ok(event) = raw_rx.try_recv() {
                            state.consumed();
                            let record = writer.raw(event, qpc()).map_err(|e| e.to_string())?;
                            notify(RecorderNotice::written(record.clone()))?;
                            if let Some(action) = writer
                                .normalize(&record, qpc())
                                .map_err(|e| e.to_string())?
                            {
                                notify(RecorderNotice::written(action))?;
                            }
                        }
                        notify(RecorderNotice::InputSuspended)?;
                    }
                    RecorderCommand::Start(_) => return Err("重复启动请求".into()),
                    RecorderCommand::Append(data) => {
                        // 暂停边界后的迟到结果不追加；父进程已在边界前记录取消结果。
                        if matches!(phase, SessionPhase::Recording | SessionPhase::Stopping) {
                            let record = writer.derived(data, qpc()).map_err(|e| e.to_string())?;
                            let related = writer
                                .associate(&record, qpc())
                                .map_err(|e| e.to_string())?;
                            notify(RecorderNotice::written(record))?;
                            if let Some(related) = related {
                                notify(RecorderNotice::written(related))?;
                            }
                        }
                    }
                    RecorderCommand::Transition(next) => {
                        if next != SessionPhase::Recording {
                            for record in writer
                                .finish_pending("会话边界前未完成证据，明确取消", qpc())
                                .map_err(|e| e.to_string())?
                            {
                                notify(RecorderNotice::written(record))?;
                            }
                        }
                        notify(RecorderNotice::written(
                            writer
                                .transition(
                                    next,
                                    format!(
                                        "用户状态边界；队列峰值{}；已知丢失{}",
                                        state.peak(),
                                        state.lost()
                                    ),
                                    qpc(),
                                )
                                .map_err(|e| e.to_string())?,
                        ))?;
                        phase = next;
                        if phase == SessionPhase::Recording {
                            state.pause(false);
                        }
                        if phase == SessionPhase::Stopped {
                            return Ok(());
                        }
                    }
                }
            }
            if synced_at.elapsed() >= Duration::from_millis(250)
                && writer.watermarks().0 > writer.watermarks().1
            {
                let (record, watermark) = writer.sync(qpc()).map_err(|e| e.to_string())?;
                notify(RecorderNotice::written(record))?;
                notify(RecorderNotice::Synced(watermark))?;
                synced_at = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    })();
    state.stop();
    let shutdown = listener.shutdown();
    if let Err(error) = &result {
        if let Ok(records) = writer.finish_pending("日志进程异常收尾，证据处理未完成", qpc())
        {
            for record in records {
                let _ = notify(RecorderNotice::written(record));
            }
        }
        if let Ok(record) = writer.transition(SessionPhase::Faulted, error.clone(), qpc()) {
            let _ = notify(RecorderNotice::written(record));
        }
        let _ = notify(RecorderNotice::Fault(error.clone()));
    }
    let final_sync = writer.sync(qpc());
    match &final_sync {
        Ok((record, watermark)) => {
            let _ = notify(RecorderNotice::written(record.clone()));
            let _ = notify(RecorderNotice::Synced(*watermark));
        }
        Err(error) => {
            let _ = notify(RecorderNotice::Fault(format!("最终同步落盘失败：{error}")));
        }
    }
    let _ = notify(RecorderNotice::Finished);
    drop(notice_tx);
    // 输出卡住不拖住进程退出；父进程托管也有终止时限。
    let deadline = Instant::now() + Duration::from_secs(1);
    while !output.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    if output.is_finished() {
        let _ = output.join();
    }
    result
        .and(shutdown)
        .and(final_sync.map(|_| ()).map_err(|e| e.to_string()))
}
