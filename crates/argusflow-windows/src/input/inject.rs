//! 物理坐标、前台校验与 SendInput 的最小原生边界。
use super::{InputAction, keyboard};
use crate::WindowsError as Failure;
use crate::{
    WindowIdentity,
    platform::{failure, hwnd},
};
use argusflow_core::{ClickCount, FailureKind, MouseButton, Operation, ScreenPoint, ScrollAxis};
use windows::Win32::{
    Foundation::{POINT, RECT},
    UI::{
        HiDpi::{
            DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            SetThreadDpiAwarenessContext,
        },
        Input::KeyboardAndMouse::*,
        WindowsAndMessaging::*,
    },
};

struct DpiGuard(DPI_AWARENESS_CONTEXT);
impl Drop for DpiGuard {
    fn drop(&mut self) {
        // SAFETY: 只恢复当前专用输入线程的 DPI 上下文。
        unsafe { SetThreadDpiAwarenessContext(self.0) };
    }
}

pub(super) fn perform(
    window: WindowIdentity,
    action: InputAction,
    operation: &Operation,
) -> Result<(), Failure> {
    operation.check("input_validate")?;
    window.require_foreground()?;
    // SAFETY: 当前线程不创建窗口，守卫在返回时恢复原上下文。
    let previous =
        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    if previous.0.is_null() {
        return Err(failure("input_dpi", windows::core::Error::from_thread()));
    }
    let _dpi = DpiGuard(previous);
    let (events, _releases) = match action {
        InputAction::Move(point) => {
            validate_point(&window, point)?;
            (vec![absolute_move(point)?], vec![])
        }
        InputAction::Click {
            point,
            button,
            count,
        } => {
            validate_point(&window, point)?;
            let (down, up, virtual_key) = match button {
                MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, VK_LBUTTON),
                MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, VK_RBUTTON),
            };
            ensure_released(virtual_key)?;
            let mut events = vec![absolute_move(point)?];
            for _ in 0..if count == ClickCount::Single { 1 } else { 2 } {
                events.push(mouse(down, 0));
                events.push(mouse(up, 0));
            }
            (events, vec![mouse(up, 0)])
        }
        InputAction::Wheel { point, axis, steps } => {
            validate_point(&window, point)?;
            if steps == 0 {
                return Err(invalid("滚轮步数不能为零"));
            }
            let delta = steps
                .checked_mul(WHEEL_DELTA as i32)
                .ok_or_else(|| invalid("滚轮步数溢出"))?;
            let flags = match axis {
                ScrollAxis::Horizontal => MOUSEEVENTF_HWHEEL,
                ScrollAxis::Vertical => MOUSEEVENTF_WHEEL,
            };
            (
                vec![absolute_move(point)?, mouse(flags, delta as u32)],
                vec![],
            )
        }
        InputAction::Text(text) => keyboard::text(&text)?,
        InputAction::Chord(keys) => keyboard::chord(&keys)?,
    };
    window.require_foreground()?;
    operation.begin_effect("send_input")?;
    super::submission::submit(&events)?;
    operation.check("input_complete").map_err(Failure::from)
}

pub(super) fn ensure_released(key: VIRTUAL_KEY) -> Result<(), Failure> {
    // SAFETY: 只读取异步键状态，拒绝释放用户原本按住的键。
    if unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0 {
        return Err(invalid("本次操作需要的按键当前已被按住"));
    }
    Ok(())
}

fn validate_point(window: &WindowIdentity, point: ScreenPoint) -> Result<(), Failure> {
    let mut bounds = RECT::default();
    // SAFETY: 窗口身份已验证，RECT 是当前线程独占输出。
    unsafe { GetWindowRect(hwnd(window.handle()), &mut bounds) }
        .map_err(|e| failure("input_bounds", e))?;
    if point.x < bounds.left
        || point.x >= bounds.right
        || point.y < bounds.top
        || point.y >= bounds.bottom
    {
        return Err(invalid("输入位置在目标窗口之外"));
    }
    // SAFETY: 只读命中测试，阻止向遮挡窗口发送输入。
    let hit = unsafe {
        WindowFromPoint(POINT {
            x: point.x,
            y: point.y,
        })
    };
    // SAFETY: 对命中窗口只查询顶层祖先。
    if unsafe { GetAncestor(hit, GA_ROOT) } != hwnd(window.handle()) {
        return Err(invalid("目标位置被其他窗口遮挡"));
    }
    Ok(())
}

fn absolute_move(point: ScreenPoint) -> Result<INPUT, Failure> {
    // SAFETY: 以下系统参数都是只读物理虚拟桌面尺寸。
    let (left, top, width, height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    let x = normalize(point.x, left, width)?;
    let y = normalize(point.y, top, height)?;
    let mut event = mouse(
        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        0,
    );
    // SAFETY: event 由 mouse 构造，tag 为 INPUT_MOUSE。
    event.Anonymous.mi.dx = x;
    event.Anonymous.mi.dy = y;
    Ok(event)
}

fn normalize(value: i32, origin: i32, extent: i32) -> Result<i32, Failure> {
    let relative = i64::from(value) - i64::from(origin);
    if extent <= 1 || relative < 0 || relative >= i64::from(extent) {
        return Err(invalid("屏幕坐标或虚拟桌面尺寸无效"));
    }
    Ok((relative * 65_535 / i64::from(extent - 1)) as i32)
}

fn mouse(flags: MOUSE_EVENT_FLAGS, data: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: flags,
                mouseData: data,
                ..Default::default()
            },
        },
    }
}

pub(super) fn invalid(message: &str) -> Failure {
    Failure::new(FailureKind::InvalidInput, "input", message)
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/input/inject.rs"]
mod tests;
