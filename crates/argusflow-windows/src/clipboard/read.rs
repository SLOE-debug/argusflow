//! Win32 剪贴板句柄仅在工作线程持有，所有退出路径释放锁。
use crate::{WindowsError, platform::failure};
use argusflow_core::{FailureKind, Operation};
use argusflow_input_contracts::{ClipboardContent, ClipboardObservation};
use windows::Win32::{
    Foundation::HGLOBAL,
    System::{DataExchange::*, Memory::*},
};

const UNICODE_TEXT: u32 = 13;
const MAX_UNITS: usize = 32768;
struct Open;
impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: 仅在 OpenClipboard 成功后创建，由同一线程释放。
        unsafe {
            let _ = CloseClipboard();
        }
    }
}
struct Locked(HGLOBAL);
impl Drop for Locked {
    fn drop(&mut self) {
        // SAFETY: 对应成功的 GlobalLock；内存归剪贴板所有，不释放数据。
        unsafe {
            let _ = GlobalUnlock(self.0);
        }
    }
}
pub(super) fn read(
    previous: Option<u32>,
    operation: &Operation,
) -> Result<ClipboardObservation, WindowsError> {
    operation.check("clipboard_read")?;
    // SAFETY: Win32 序号查询不转移资源所有权。
    let sequence = unsafe { GetClipboardSequenceNumber() };
    if sequence == 0 {
        return Err(WindowsError::new(
            FailureKind::Unavailable,
            "clipboard_sequence",
            "剪贴板序号不可用",
        ));
    }
    if previous == Some(sequence) {
        return Ok(ClipboardObservation {
            previous_sequence: previous,
            sequence,
            content: ClipboardContent::Unchanged,
        });
    }
    // 不重试占用锁；下一次独立观察可以再次读取。
    unsafe { OpenClipboard(None) }.map_err(|e| failure("clipboard_open", e))?;
    let _open = Open;
    let content = if unsafe { IsClipboardFormatAvailable(UNICODE_TEXT) }.is_ok() {
        operation.check("clipboard_content")?;
        // GetClipboardData 可触发延迟渲染，故只在单独工作线程执行。
        let handle =
            unsafe { GetClipboardData(UNICODE_TEXT) }.map_err(|e| failure("clipboard_data", e))?;
        let global = HGLOBAL(handle.0);
        let bytes = unsafe { GlobalSize(global) };
        if bytes < 2 || bytes % 2 != 0 {
            return Err(WindowsError::new(
                FailureKind::Protocol,
                "clipboard_size",
                "Unicode 剪贴板数据长度无效",
            ));
        }
        let pointer = unsafe { GlobalLock(global) };
        if pointer.is_null() {
            return Err(failure(
                "clipboard_lock",
                windows::core::Error::from_thread(),
            ));
        }
        let _lock = Locked(global);
        // SAFETY: 已验证 HGLOBAL 大小并锁定；只借用有界区域，不读取超出分配的数据。
        let units = unsafe {
            std::slice::from_raw_parts(pointer.cast::<u16>(), (bytes / 2).min(MAX_UNITS + 1))
        };
        let terminal = units.iter().position(|v| *v == 0);
        if terminal.is_none() && bytes / 2 <= MAX_UNITS {
            return Err(WindowsError::new(
                FailureKind::Protocol,
                "clipboard_unicode",
                "Unicode 剪贴板缺少终止符",
            ));
        }
        let truncated = terminal.is_none_or(|n| n > MAX_UNITS);
        let mut end = terminal.unwrap_or(units.len()).min(MAX_UNITS);
        if truncated && end > 0 && (0xD800..=0xDBFF).contains(&units[end - 1]) {
            end -= 1;
        }
        let text = String::from_utf16(&units[..end]).map_err(|e| {
            WindowsError::new(
                FailureKind::Protocol,
                "clipboard_unicode",
                "剪贴板包含无效 UTF-16",
            )
            .with_source(e)
        })?;
        ClipboardContent::Text { text, truncated }
    } else {
        ClipboardContent::NoText
    };
    if unsafe { GetClipboardSequenceNumber() } != sequence {
        return Err(WindowsError::new(
            FailureKind::StaleHandle,
            "clipboard_sequence",
            "读取期间剪贴板发生变化",
        ));
    }
    operation.check("clipboard_complete")?;
    Ok(ClipboardObservation {
        previous_sequence: previous,
        sequence,
        content,
    })
}
