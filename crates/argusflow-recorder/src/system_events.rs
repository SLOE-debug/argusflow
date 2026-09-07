//! 同一 Hook 消息线程拥有的窗口事件与剪贴板通知，不使用轮询推测变化。

use crate::{PhysicalInput, RecorderError, WindowChange, hooks::emit};
use argusflow_core::WindowIdentity;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{
            DataExchange::{
                AddClipboardFormatListener, GetClipboardSequenceNumber,
                RemoveClipboardFormatListener,
            },
            LibraryLoader::GetModuleHandleW,
            SystemInformation::GetTickCount,
        },
        UI::{
            Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
            WindowsAndMessaging::*,
        },
    },
    core::w,
};

/// 所有资源由安装线程销毁；失败时自动卸载已经注册的监听。
pub(crate) struct SystemEventCapture {
    /// 仅监听前台顶层窗口变化。
    foreground: HWINEVENTHOOK,
    /// 仅监听原生窗口显示通知。
    appeared: HWINEVENTHOOK,
    /// 接收剪贴板通知的不可见 message-only 窗口。
    clipboard_window: HWND,
}

impl SystemEventCapture {
    pub(crate) fn install() -> Result<Self, RecorderError> {
        let mut capture = Self {
            foreground: HWINEVENTHOOK::default(),
            appeared: HWINEVENTHOOK::default(),
            clipboard_window: HWND::default(),
        };
        // SAFETY: out-of-context 回调返回安装线程；不注入其他进程。
        capture.foreground = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(window_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        capture.appeared = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_SHOW,
                EVENT_OBJECT_SHOW,
                None,
                Some(window_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if capture.foreground.is_invalid() || capture.appeared.is_invalid() {
            return Err(RecorderError::HookUnavailable);
        }
        let instance =
            unsafe { GetModuleHandleW(None) }.map_err(|_| RecorderError::HookUnavailable)?;
        let class_name = w!("ArgusFlowRecorderClipboard");
        let class = WNDCLASSW {
            lpfnWndProc: Some(clipboard_message),
            hInstance: instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        // 类可能已由同进程上次录制注册；CreateWindowExW 决定实际安装结果。
        unsafe { RegisterClassW(&class) };
        capture.clipboard_window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                w!(""),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance.into()),
                None,
            )
        }
        .map_err(|_| RecorderError::HookUnavailable)?;
        unsafe { AddClipboardFormatListener(capture.clipboard_window) }
            .map_err(|_| RecorderError::HookUnavailable)?;
        Ok(capture)
    }
}

impl Drop for SystemEventCapture {
    fn drop(&mut self) {
        // SAFETY: 只释放本次安装取得的句柄，消息窗没有可见 UI。
        unsafe {
            if !self.foreground.is_invalid() {
                let _ = UnhookWinEvent(self.foreground);
            }
            if !self.appeared.is_invalid() {
                let _ = UnhookWinEvent(self.appeared);
            }
            if !self.clipboard_window.is_invalid() {
                let _ = RemoveClipboardFormatListener(self.clipboard_window);
                let _ = DestroyWindow(self.clipboard_window);
            }
        }
    }
}

unsafe extern "system" fn window_event(
    _: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    object: i32,
    child: i32,
    _: u32,
    time: u32,
) {
    if window.is_invalid() || object != OBJID_WINDOW.0 || child != 0 {
        return;
    }
    // 只保留实际顶层窗口，子控件 show 不冒充应用出现。
    if unsafe { GetAncestor(window, GA_ROOT) } != window {
        return;
    }
    let change = match event {
        EVENT_SYSTEM_FOREGROUND => WindowChange::Foreground,
        EVENT_OBJECT_SHOW => WindowChange::Appeared,
        _ => return,
    };
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    if process_id != 0 {
        emit(
            time,
            PhysicalInput::Window {
                window: WindowIdentity {
                    handle: window.0 as u64,
                    process_id,
                },
                change,
            },
        );
    }
}

unsafe extern "system" fn clipboard_message(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CLIPBOARDUPDATE {
        // WM_CLIPBOARDUPDATE 无内置时间/版本，记录本次通知送达时的系统时钟与版本。
        emit(
            unsafe { GetTickCount() },
            PhysicalInput::Clipboard {
                sequence_number: unsafe { GetClipboardSequenceNumber() },
            },
        );
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
