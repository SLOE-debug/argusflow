//! 被比较的最小策略；每种独立执行，失败不串到另一种方法。
use super::{Result, model::Strategy};
use argusflow_core::{Key, OperationOptions};
use argusflow_windows::{InputAction, InputService, WindowIdentity};
use windows::Win32::{
    Foundation::*,
    System::Threading::{AttachThreadInput, GetCurrentThreadId},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

pub async fn execute(
    strategy: Strategy,
    target: &WindowIdentity,
    cover: &WindowIdentity,
    input: &InputService,
) -> Result<Vec<String>> {
    let hwnd = HWND(target.handle() as *mut _);
    let mut trace = Vec::new();
    // 正式服务自己承担恢复和稳定性验证，不能被 demo 的共同恢复步骤代替。
    if matches!(strategy, Strategy::Project) {
        input
            .perform(
                target.clone(),
                InputAction::ActivateWindow,
                OperationOptions::default(),
            )
            .await?;
        return Ok(vec![
            "project mouse move + SetForegroundWindow + stable foreground verification".into(),
        ]);
    }
    // 所有策略使用同一恢复步骤；独立记录恢复后已获得前台的情况。
    unsafe {
        if IsIconic(hwnd).as_bool() {
            trace.push(format!(
                "ShowWindowAsync(SW_RESTORE)={:?}",
                ShowWindowAsync(hwnd, SW_RESTORE)
            ));
            for _ in 0..50 {
                if !IsIconic(hwnd).as_bool() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        trace.push(format!(
            "foreground_after_restore={}",
            GetForegroundWindow().0 as isize
        ));
    }
    if unsafe { GetForegroundWindow() } == hwnd {
        trace.push("restore already activated target; no further input".into());
        return Ok(trace);
    }
    match strategy {
        Strategy::Direct => trace.push(set_foreground(hwnd)),
        Strategy::Alt => {
            ensure_modifiers_released()?;
            // 使用项目输入服务执行一次完整 Alt 按下/释放，只作用于自有前台覆盖窗口。
            cover.require_foreground()?;
            input
                .perform(
                    cover.clone(),
                    InputAction::Chord(vec![Key::Alt]),
                    OperationOptions::default(),
                )
                .await?;
            trace.push("project SendInput Alt down/up".into());
            trace.push(set_foreground(hwnd));
        }
        Strategy::MouseMove => {
            cover.require_foreground()?;
            // pywinauto 风格：鼠标移向桌面左缘后请求激活；不点击、不发送业务键。
            // 物理虚拟桌面坐标归一化，禁止依赖当前进程 DPI 缩放。
            let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
            let event = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: (500i64.min(i64::from(height - 1)) * 65535 / i64::from(height - 1))
                            as i32,
                        dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                        dwExtraInfo: argusflow_input_contracts::OWN_INPUT_TAG,
                        ..Default::default()
                    },
                },
            };
            let count = unsafe { SendInput(&[event], std::mem::size_of::<INPUT>() as i32) };
            if count != 1 {
                return Err("鼠标移动未完整注入".into());
            }
            trace.push("SendInput mouse move to virtual desktop left edge".into());
            trace.push(set_foreground(hwnd));
        }
        Strategy::Attach => {
            // 独立 worker 承担阻塞风险；父进程有硬超时，不在主执行器内实验。
            let current = unsafe { GetCurrentThreadId() };
            let foreground = unsafe { GetWindowThreadProcessId(GetForegroundWindow(), None) };
            let mut message = MSG::default();
            unsafe {
                let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
            }
            if current == foreground || foreground == 0 {
                return Err("Attach 前置线程关系无效".into());
            }
            unsafe { AttachThreadInput(current, foreground, true) }.ok()?;
            trace.push("AttachThreadInput(current, foreground, true)".into());
            trace.push(set_foreground(hwnd));
            unsafe { AttachThreadInput(current, foreground, false) }.ok()?;
            trace.push("AttachThreadInput detached".into());
        }
        Strategy::Switch => {
            // 微软不承诺此接口供通用使用，本 demo 仅记录实际表现。
            unsafe { SwitchToThisWindow(hwnd, true) };
            trace.push("SwitchToThisWindow(hwnd, TRUE), void return".into());
        }
        Strategy::Project => unreachable!("正式服务在共同恢复步骤前执行"),
    }
    Ok(trace)
}
fn set_foreground(hwnd: HWND) -> String {
    // SAFETY: worker 在调用前通过 WindowIdentity 验证 HWND 和进程身份。
    format!(
        "SetForegroundWindow={}",
        unsafe { SetForegroundWindow(hwnd) }.as_bool()
    )
}
pub fn ensure_modifiers_released() -> Result<()> {
    for key in [
        VK_MENU, VK_CONTROL, VK_SHIFT, VK_LWIN, VK_RWIN, VK_LBUTTON, VK_RBUTTON,
    ] {
        if unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0 {
            return Err("用户正在按住修饰键或鼠标键，未执行本轮".into());
        }
    }
    Ok(())
}
