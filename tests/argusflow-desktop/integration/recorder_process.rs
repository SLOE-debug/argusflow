//! 使用真实子进程协议；监听范围与排除范围均为测试父进程，不捕获日常应用。
use argusflow_recorder::*;
use std::os::windows::process::CommandExt;
use std::{
    io::{BufRead, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
struct Host {
    child: Child,
    input: Option<ChildStdin>,
    notices: mpsc::Receiver<RecorderNotice>,
    _temp: tempfile::TempDir,
}
impl Host {
    fn start() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("session");
        let mut child = Command::new(env!("CARGO_BIN_EXE_argusflow"))
            .arg("--recorder-child")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, notices) = mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                let notice = serde_json::from_str(&line).unwrap();
                if tx.send(notice).is_err() {
                    break;
                }
            }
        });
        let mut host = Self {
            child,
            input: Some(input),
            notices,
            _temp: temp,
        };
        host.send(RecorderCommand::Start(StartRequest {
            directory: directory.to_string_lossy().into_owned(),
            session: Session {
                id: "process-test".into(),
                format: 2,
                created_ms: 0,
                qpc_origin: argusflow_windows::listening::qpc(),
                qpc_frequency: argusflow_windows::listening::qpc_frequency().unwrap(),
                capture_session: None,
                policy: "isolated-process-test".into(),
            },
            excluded_pid: std::process::id(),
            included_pid: Some(std::process::id()),
        }));
        host.until(|notice| matches!(notice, RecorderNotice::Ready));
        host.phase(SessionPhase::Recording);
        host
    }
    fn send(&mut self, command: RecorderCommand) {
        let input = self.input.as_mut().unwrap();
        serde_json::to_writer(&mut *input, &command).unwrap();
        input.write_all(b"\n").unwrap();
        input.flush().unwrap();
    }
    fn until(&self, condition: impl Fn(&RecorderNotice) -> bool) {
        loop {
            let notice = self.notices.recv_timeout(Duration::from_secs(8)).unwrap();
            if condition(&notice) {
                break;
            }
            assert!(!matches!(notice, RecorderNotice::Fault(_)), "{notice:?}");
        }
    }
    fn phase(&mut self, phase: SessionPhase) {
        self.send(RecorderCommand::Transition(phase));
        self.until(|notice|matches!(notice,RecorderNotice::Written(record) if matches!(record.data,RecordData::State{phase:observed,..} if observed==phase)));
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[test]
fn child_handshake_pause_resume_stop_and_durability() {
    let mut host = Host::start();
    host.send(RecorderCommand::SuspendInput);
    host.until(|notice| matches!(notice, RecorderNotice::InputSuspended));
    host.phase(SessionPhase::Paused);
    host.phase(SessionPhase::Recording);
    host.phase(SessionPhase::Stopping);
    host.phase(SessionPhase::Stopped);
    host.until(|notice| matches!(notice, RecorderNotice::Finished));
    assert!(host.child.wait().unwrap().success());
    let page = read_page(&host._temp.path().join("session"), Cursor::default(), 128).unwrap();
    assert!(page.tail.is_none());
    assert!(
        !page
            .records
            .iter()
            .any(|r| matches!(r.data, RecordData::Raw(_)))
    );
    assert!(
        page.records
            .iter()
            .any(|r| matches!(r.data,RecordData::Durability{through} if through>0))
    );
}
#[test]
fn parent_pipe_loss_ends_child_and_retains_prefix() {
    let mut host = Host::start();
    host.input.take();
    let deadline = std::time::Instant::now() + Duration::from_secs(6);
    while host.child.try_wait().unwrap().is_none() {
        assert!(std::time::Instant::now() < deadline, "orphan child");
        std::thread::sleep(Duration::from_millis(20));
    }
    let page = read_page(&host._temp.path().join("session"), Cursor::default(), 128).unwrap();
    assert!(page.records.iter().any(|r| matches!(
        r.data,
        RecordData::State {
            phase: SessionPhase::Faulted,
            ..
        }
    )));
}

#[test]
#[ignore = "被动保存当前桌面；使用真实打包EXE录制两个片段，不注入输入"]
fn packaged_video_segments_close_and_decode() {
    let mut host = Host::start();
    let recording = host._temp.path().join("session");
    std::fs::create_dir(recording.join("video")).unwrap();
    for segment in ["000001", "000002"] {
        let path = recording.join("video").join(segment);
        let mut video = Command::new(env!("CARGO_BIN_EXE_argusflow"))
            .arg("--recorder-video")
            .arg(&path)
            .arg("268435456")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        while !path.join("ready.json").exists() {
            host.send(RecorderCommand::Heartbeat);
            if let Some(status) = video.try_wait().unwrap() {
                panic!("video start failed {status}");
            }
            if std::time::Instant::now() >= deadline {
                let _ = video.kill();
                let _ = video.wait();
                panic!("video ready timeout");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        for _ in 0..12 {
            host.send(RecorderCommand::Heartbeat);
            std::thread::sleep(Duration::from_millis(100));
        }
        video.stdin.take().unwrap().write_all(b"S").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(6);
        loop {
            host.send(RecorderCommand::Heartbeat);
            if let Some(status) = video.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if std::time::Instant::now() >= deadline {
                let _ = video.kill();
                let _ = video.wait();
                panic!("video stop timeout");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(path.join("complete.json").exists());
        let text = std::fs::read_to_string(path.join("frames.jsonl")).unwrap();
        let entry: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
        let pts = entry["frame"]["pts_100ns"].as_i64().unwrap();
        let frame = argusflow_windows::decode_video_frame(&path.join("screen.mp4"), pts).unwrap();
        assert!(!frame.rgba.is_empty());
        host.phase(SessionPhase::Paused);
        if segment == "000001" {
            host.phase(SessionPhase::Recording);
        }
    }
    host.phase(SessionPhase::Stopping);
    host.phase(SessionPhase::Stopped);
    host.until(|n| matches!(n, RecorderNotice::Finished));
}
