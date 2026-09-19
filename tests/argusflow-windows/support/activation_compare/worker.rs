//! 每次尝试在独立进程中执行，读取真实前台、菜单、光标和按键状态。
use super::{
    Result,
    model::{Attempt, Snapshot, Strategy},
    strategies,
};
use argusflow_windows::{InputService, OperationOptions, WindowIdentity};
use std::time::{Duration, Instant};
use windows::Win32::{
    Foundation::*,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

pub async fn run(strategy: Strategy, target: isize, cover: isize, output: &str) -> Result<()> {
    let target_window = WindowIdentity::from_handle(target)?;
    let cover_window = WindowIdentity::from_handle(cover)?;
    cover_window.require_foreground()?;
    strategies::ensure_modifiers_released()?;
    let input = InputService::new()?;
    let before = snapshot(target)?;
    let started = Instant::now();
    let result = strategies::execute(strategy, &target_window, &cover_window, &input).await;
    let api_ms = started.elapsed().as_secs_f64() * 1000.0;
    // 成功要求目标成为前台且连续保持 150ms；只等待状态，不再次调用激活 API。
    let deadline = Instant::now() + Duration::from_millis(700);
    let mut stable_since = None;
    let mut stable = false;
    while Instant::now() < deadline {
        if unsafe { GetForegroundWindow() }.0 as isize == target {
            let since = stable_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= Duration::from_millis(150) {
                stable = true;
                break;
            }
        } else {
            stable_since = None;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let after = snapshot(target)?;
    let (api_trace, error) = match result {
        Ok(trace) => (trace, None),
        Err(error) => (vec![], Some(error.to_string())),
    };
    let attempt = Attempt {
        strategy,
        target,
        worker_pid: std::process::id(),
        before,
        after,
        api_ms,
        elapsed_ms,
        foreground_stable: stable,
        api_trace,
        error,
    };
    input.shutdown(OperationOptions::default()).await?;
    std::fs::write(output, serde_json::to_vec_pretty(&attempt)?)?;
    Ok(())
}
pub fn snapshot(target: isize) -> Result<Snapshot> {
    // SAFETY: 以下均是同步只读查询；不读取其他应用的文本或剪贴板。
    unsafe {
        let foreground = GetForegroundWindow();
        let mut pid = 0;
        let thread = GetWindowThreadProcessId(foreground, Some(&mut pid));
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        GetGUIThreadInfo(thread, &mut info)?;
        let mut cursor = POINT::default();
        GetCursorPos(&mut cursor)?;
        let hwnd = HWND(target as *mut _);
        Ok(Snapshot {
            foreground: foreground.0 as isize,
            foreground_pid: pid,
            focus: info.hwndFocus.0 as isize,
            gui_flags: info.flags.0,
            cursor: [cursor.x, cursor.y],
            alt_down: GetAsyncKeyState(i32::from(VK_MENU.0)) < 0,
            target_topmost: GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TOPMOST.0 as isize != 0,
            target_minimized: IsIconic(hwnd).as_bool(),
        })
    }
}
