//! 顺序运行独立 worker；每轮重建前台输入条件，超时仅终止自有 worker。
use super::{
    Result, fixture,
    model::{Attempt, Strategy, Trial},
    strategies,
};
use argusflow_core::ScreenPoint;
use argusflow_windows::{
    InputAction, InputService, OperationOptions, WindowIdentity, WindowLocator,
};
use std::{
    io::Write,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::*, System::Threading::CREATE_NO_WINDOW, UI::WindowsAndMessaging::*,
};

struct OwnedFixture {
    child: Child,
    window: WindowIdentity,
}
impl Drop for OwnedFixture {
    fn drop(&mut self) {
        // 只回收当前 demo 创建的进程，不关闭记事本或微信。
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
struct Case {
    name: String,
    window: WindowIdentity,
    minimized: bool,
}

pub async fn run(output: &str, rounds: usize, real: bool) -> Result<()> {
    if !(1..=20).contains(&rounds) {
        return Err("轮数应为 1..=20".into());
    }
    let output = PathBuf::from(output);
    std::fs::create_dir(&output)?;
    let output = output.canonicalize()?;
    let exe = std::env::current_exe()?;
    let cover = spawn_fixture(&exe, &output, "cover").await?;
    let normal = spawn_fixture(&exe, &output, "normal").await?;
    let tool = spawn_fixture(&exe, &output, "tool").await?;
    let frameless = spawn_fixture(&exe, &output, "frameless").await?;
    let mut cases = vec![
        Case {
            name: "normal".into(),
            window: normal.window.clone(),
            minimized: false,
        },
        Case {
            name: "tool_no_taskbar".into(),
            window: tool.window.clone(),
            minimized: false,
        },
        Case {
            name: "frameless_no_taskbar".into(),
            window: frameless.window.clone(),
            minimized: false,
        },
        Case {
            name: "minimized".into(),
            window: normal.window.clone(),
            minimized: true,
        },
    ];
    let mut skipped = Vec::new();
    if real {
        for (name, locator) in [
            (
                "notepad",
                WindowLocator {
                    class_name: Some("Notepad".into()),
                    ..Default::default()
                },
            ),
            (
                "wechat",
                WindowLocator {
                    title: Some("微信".into()),
                    ..Default::default()
                },
            ),
        ] {
            match locator.find_unique() {
                Ok(w) => cases.push(Case {
                    name: name.into(),
                    window: w.identity(),
                    minimized: false,
                }),
                Err(error) => skipped.push(format!("{name}: {error}")),
            }
        }
    }
    std::fs::write(
        output.join("environment.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"rounds":rounds,"real":real,"skipped":skipped,"cases":cases.iter().map(|c|serde_json::json!({"name":c.name,"hwnd":c.window.handle(),"pid":c.window.process_id()})).collect::<Vec<_>>(),"method":"fresh worker per trial; cover receives a fresh physical click; normal and explicit LSFW_LOCK; exact HWND stable 150ms; worker hard timeout 4s"}),
        )?,
    )?;
    let input = InputService::new()?;
    let result = compare(&exe, &output, rounds, &cases, &cover.window, &input).await;
    let _ = fixture::control(cover.window.handle(), fixture::RELEASE);
    let cleanup = input.shutdown(OperationOptions::default()).await;
    result?;
    cleanup?;
    Ok(())
}
async fn compare(
    exe: &Path,
    output: &Path,
    rounds: usize,
    cases: &[Case],
    cover: &WindowIdentity,
    input: &InputService,
) -> Result<()> {
    let mut journal = std::fs::File::create(output.join("trials.jsonl"))?;
    let mut trials = Vec::new();
    for round in 0..rounds {
        for case in cases {
            for locked in [false, true] {
                for offset in 0..Strategy::ALL.len() {
                    let strategy = Strategy::ALL[(offset + round) % Strategy::ALL.len()];
                    strategies::ensure_modifiers_released()?;
                    fixture::control(cover.handle(), fixture::RELEASE)?;
                    if case.minimized {
                        unsafe {
                            ShowWindowAsync(HWND(case.window.handle() as *mut _), SW_MINIMIZE)
                        }
                        .ok()?;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    if !fixture::control(cover.handle(), fixture::REVEAL)? {
                        return Err("测试覆盖窗口未显露".into());
                    }
                    // 即使覆盖窗口原本就在前台，也发送新的真实点击，清掉上一轮输入条件。
                    let mut rect = RECT::default();
                    unsafe {
                        use windows::Win32::UI::HiDpi::*;
                        let previous = SetThreadDpiAwarenessContext(
                            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                        );
                        let result = GetWindowRect(HWND(cover.handle() as *mut _), &mut rect);
                        SetThreadDpiAwarenessContext(previous);
                        result?;
                    }
                    input
                        .perform(
                            cover.clone(),
                            InputAction::FocusClick(ScreenPoint {
                                x: (rect.left + rect.right) / 2,
                                y: (rect.top + rect.bottom) / 2,
                            }),
                            OperationOptions::default(),
                        )
                        .await?;
                    tokio::time::sleep(Duration::from_millis(60)).await;
                    cover.require_foreground()?;
                    if !fixture::control(cover.handle(), fixture::NORMAL_Z)? {
                        return Err("测试覆盖窗口置顶状态未还原".into());
                    }
                    let lock_accepted = if locked {
                        fixture::control(cover.handle(), fixture::ARM)?
                    } else {
                        false
                    };
                    if locked && !lock_accepted {
                        return Err("测试前台锁未能建立，不能计入比较".into());
                    }
                    let file = output.join(format!(
                        "{}-{}-{}-{}.json",
                        round + 1,
                        case.name,
                        if locked { "locked" } else { "normal" },
                        strategy.name()
                    ));
                    let mut child = command(exe)
                        .args([
                            "worker",
                            strategy.name(),
                            &case.window.handle().to_string(),
                            &cover.handle().to_string(),
                        ])
                        .arg(&file)
                        .spawn()?;
                    let deadline = Instant::now() + Duration::from_secs(4);
                    let mut timeout = false;
                    let status = loop {
                        if let Some(status) = child.try_wait()? {
                            break Some(status);
                        }
                        if Instant::now() >= deadline {
                            child.kill()?;
                            child.wait()?;
                            timeout = true;
                            break None;
                        }
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    };
                    let (attempt, error) = match std::fs::read(&file) {
                        Ok(bytes) => (Some(serde_json::from_slice::<Attempt>(&bytes)?), None),
                        Err(error) => (None, Some(format!("worker={status:?}; {error}"))),
                    };
                    let trial = Trial {
                        case: case.name.clone(),
                        restriction: if locked { "locked" } else { "normal" }.into(),
                        round: round + 1,
                        strategy,
                        prepared_foreground: cover.handle(),
                        lock_accepted,
                        attempt,
                        worker_timeout: timeout,
                        error,
                    };
                    writeln!(journal, "{}", serde_json::to_string(&trial)?)?;
                    journal.flush()?;
                    trials.push(trial);
                }
                let recent = &trials[trials.len() - Strategy::ALL.len()..];
                let summary = recent
                    .iter()
                    .map(|t| {
                        format!(
                            "{}={}",
                            t.strategy.name(),
                            if t.attempt
                                .as_ref()
                                .is_some_and(|a| a.foreground_stable && a.error.is_none())
                            {
                                "ok"
                            } else {
                                "FAIL"
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                println!(
                    "round={} case={} locked={} {summary}",
                    round + 1,
                    case.name,
                    locked
                );
            }
        }
    }
    std::fs::write(
        output.join("results.json"),
        serde_json::to_vec_pretty(&trials)?,
    )?;
    println!("{} trials saved: {}", trials.len(), output.display());
    Ok(())
}
fn command(exe: &Path) -> Command {
    let mut command = Command::new(exe);
    command
        .creation_flags(CREATE_NO_WINDOW.0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    command
}
async fn spawn_fixture(exe: &Path, output: &Path, kind: &str) -> Result<OwnedFixture> {
    let ready = output.join(format!("fixture-{kind}.json"));
    let mut child = command(exe).args(["fixture", kind]).arg(&ready).spawn()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let result = async {
        loop {
            if ready.is_file()
                && let Ok(data) =
                    serde_json::from_slice::<serde_json::Value>(&std::fs::read(&ready)?)
            {
                let handle = data["hwnd"].as_i64().ok_or("窗口句柄缺失")? as isize;
                return Ok::<_, Box<dyn std::error::Error>>(WindowIdentity::from_handle(handle)?);
            }
            if child.try_wait()?.is_some() {
                return Err("自有测试窗口提前退出".into());
            }
            if Instant::now() >= deadline {
                return Err("测试窗口启动超时".into());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    .await;
    match result {
        Ok(window) => Ok(OwnedFixture { child, window }),
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(error)
        }
    }
}
