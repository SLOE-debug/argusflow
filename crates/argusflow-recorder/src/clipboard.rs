//! 只读取通知对应版本的 Unicode 剪贴板；不读取未变化的初始剪贴板。

use crate::ClipboardContent;
use windows::Win32::{
    Foundation::HGLOBAL,
    System::{
        DataExchange::*,
        Memory::{GlobalLock, GlobalSize, GlobalUnlock},
    },
};

/// 在摄入线程同步复制，后续异步查询不再访问 live clipboard。
pub(crate) fn capture(sequence_number: u32) -> ClipboardContent {
    // SAFETY: 仅查询版本与读取数据，不修改剪贴板内容。
    if unsafe { GetClipboardSequenceNumber() } != sequence_number {
        return ClipboardContent::ChangedBeforeCapture;
    }
    if unsafe { OpenClipboard(None) }.is_err() {
        return ClipboardContent::Unavailable;
    }
    let _guard = ClipboardGuard;
    if unsafe { GetClipboardSequenceNumber() } != sequence_number {
        return ClipboardContent::ChangedBeforeCapture;
    }
    // CF_UNICODETEXT 的标准 Win32 格式值，不使用应用自定义格式。
    const UNICODE_TEXT: u32 = 13;
    if unsafe { IsClipboardFormatAvailable(UNICODE_TEXT) }.is_err() {
        return if unsafe { CountClipboardFormats() } == 0 {
            ClipboardContent::Empty
        } else {
            ClipboardContent::NonText
        };
    }
    let Ok(handle) = (unsafe { GetClipboardData(UNICODE_TEXT) }) else {
        return ClipboardContent::Unavailable;
    };
    let allocation = HGLOBAL(handle.0);
    let length = unsafe { GlobalSize(allocation) } / 2;
    let pointer = unsafe { GlobalLock(allocation) } as *const u16;
    if pointer.is_null() {
        return ClipboardContent::Unavailable;
    }
    // 数据在 OpenClipboard + GlobalLock 范围内有效，最多读取 64K UTF-16 单元。
    let text = unsafe { std::slice::from_raw_parts(pointer, length.min(65_536)) };
    let end = text.iter().position(|unit| *unit == 0);
    let result = ClipboardContent::Text {
        value: String::from_utf16_lossy(&text[..end.unwrap_or(text.len())]),
        truncated: end.is_none(),
    };
    unsafe {
        let _ = GlobalUnlock(allocation);
    }
    if unsafe { GetClipboardSequenceNumber() } != sequence_number {
        ClipboardContent::ChangedBeforeCapture
    } else {
        result
    }
}

/// 包含所有早退分支的剪贴板锁生命周期。
struct ClipboardGuard;
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}
