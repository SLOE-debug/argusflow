//! Windows SendInput 鼠标点击与滚轮注入。

use std::mem::size_of;

use argusflow_agent::WindowContext;
use argusflow_core::ScreenPoint;
use argusflow_vision::WheelSteps;
use thiserror::Error;
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::{
        Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_MOUSE, MOUSE_EVENT_FLAGS, MOUSEEVENTF_ABSOLUTE,
            MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK,
            MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
        },
        WindowsAndMessaging::{
            GetSystemMetrics, GetWindowRect, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
            SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
        },
    },
};

use super::keyboard::ensure_foreground_window;

/// 鼠标 SendInput 的前置校验或提交失败。
#[derive(Debug, Error)]
pub enum MouseInputError {
    /// HWND/PID/前台复验失败。
    #[error("鼠标输入窗口校验失败: {0}")]
    WindowValidation(String),
    /// 目标点不在已复验目标窗口内。
    #[error("目标点不在目标窗口范围内")]
    PointOutsideWindow,
    /// Windows 虚拟桌面尺寸不可用于绝对坐标换算。
    #[error("Windows 虚拟桌面尺寸无效")]
    InvalidVirtualDesktop,
    /// wheel 步数换算为 mouseData 时溢出。
    #[error("滚轮步数超出 SendInput 范围")]
    WheelOverflow,
    /// UIPI 或系统输入队列只接受了部分事件。
    #[error("Windows 只注入了 {inserted}/{requested} 个鼠标事件")]
    PartialInjection { inserted: u32, requested: usize },
    /// 读取窗口矩形失败。
    #[error("读取目标窗口矩形失败: {0}")]
    WindowBounds(String),
}

/// 对已冻结窗口执行一次经过点位复验的左键点击。
pub(super) fn inject_click(
    window: &WindowContext,
    point: ScreenPoint,
) -> Result<(), MouseInputError> {
    ensure_foreground_window(window)
        .map_err(|error| MouseInputError::WindowValidation(error.to_string()))?;
    ensure_point_in_window(window, point)?;
    let move_input = absolute_move(point)?;
    let inputs = [
        move_input,
        mouse_input(MOUSEEVENTF_LEFTDOWN, 0),
        mouse_input(MOUSEEVENTF_LEFTUP, 0),
    ];
    inject_inputs(&inputs)
}

/// 将鼠标移到滚动区域安全点并注入一批滚轮步数。
pub fn inject_scroll_wheel(
    window: &WindowContext,
    point: ScreenPoint,
    steps: WheelSteps,
) -> Result<(), MouseInputError> {
    ensure_foreground_window(window)
        .map_err(|error| MouseInputError::WindowValidation(error.to_string()))?;
    ensure_point_in_window(window, point)?;
    let wheel_data = i64::from(steps.get())
        .checked_mul(120)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or(MouseInputError::WheelOverflow)?;
    let inputs = [
        absolute_move(point)?,
        mouse_input(MOUSEEVENTF_WHEEL, wheel_data as u32),
    ];
    inject_inputs(&inputs)
}

/// 确认点位仍属于目标 HWND，避免点击到前台窗口的其它区域。
fn ensure_point_in_window(
    window: &WindowContext,
    point: ScreenPoint,
) -> Result<(), MouseInputError> {
    let hwnd = native_window(window.handle);
    let mut bounds = RECT::default();
    // SAFETY: bounds 是同步 Win32 调用的独占输出，HWND 已由前置函数复验。
    unsafe { GetWindowRect(hwnd, &mut bounds) }
        .map_err(|error| MouseInputError::WindowBounds(error.to_string()))?;
    if point.x < bounds.left
        || point.y < bounds.top
        || point.x >= bounds.right
        || point.y >= bounds.bottom
    {
        return Err(MouseInputError::PointOutsideWindow);
    }
    Ok(())
}

/// 把虚拟桌面物理像素换算成 SendInput 绝对坐标。
fn absolute_move(point: ScreenPoint) -> Result<INPUT, MouseInputError> {
    // SAFETY: GetSystemMetrics is a read-only query with a closed enum argument.
    let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    // SAFETY: GetSystemMetrics is a read-only query with a closed enum argument.
    let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    // SAFETY: GetSystemMetrics is a read-only query with a closed enum argument.
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    // SAFETY: GetSystemMetrics is a read-only query with a closed enum argument.
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    if width <= 1 || height <= 1 {
        return Err(MouseInputError::InvalidVirtualDesktop);
    }
    let x = normalize_coordinate(point.x, left, width)?;
    let y = normalize_coordinate(point.y, top, height)?;
    Ok(INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: x,
                dy: y,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                ..MOUSEINPUT::default()
            },
        },
    })
}

/// 使用整数算术执行带边界检查的绝对坐标归一化。
fn normalize_coordinate(value: i32, origin: i32, extent: i32) -> Result<i32, MouseInputError> {
    let upper = origin
        .checked_add(extent)
        .ok_or(MouseInputError::InvalidVirtualDesktop)?;
    if value < origin || value >= upper {
        return Err(MouseInputError::PointOutsideWindow);
    }
    let numerator = (i64::from(value) - i64::from(origin)) * 65_535;
    let denominator = i64::from(extent - 1);
    i32::try_from(numerator / denominator).map_err(|_| MouseInputError::InvalidVirtualDesktop)
}

/// 构造单个鼠标事件。
fn mouse_input(flags: MOUSE_EVENT_FLAGS, data: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                mouseData: data,
                dwFlags: flags,
                ..MOUSEINPUT::default()
            },
        },
    }
}

/// 一次性提交完整鼠标事件序列，拒绝把部分注入误报为成功。
fn inject_inputs(inputs: &[INPUT]) -> Result<(), MouseInputError> {
    if inputs.is_empty() {
        return Ok(());
    }
    // SAFETY: INPUT slice 在同步调用期间有效，结构尺寸来自同一 windows crate 类型。
    let inserted = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if inserted as usize != inputs.len() {
        return Err(MouseInputError::PartialInjection {
            inserted,
            requested: inputs.len(),
        });
    }
    Ok(())
}

/// 把领域层窗口句柄恢复为 Win32 类型。
fn native_window(handle: u64) -> HWND {
    HWND(handle as usize as *mut std::ffi::c_void)
}
