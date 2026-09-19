//! 只读系统剪贴板；仅在复制后序号变化时由采集端读取。
use super::Result;
use windows::Win32::System::{DataExchange::*, Memory::*};
pub fn sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}
pub fn read() -> Result<String> {
    // SAFETY: 只在此同步调用持有剪贴板，不跨 await，不修改任何格式。
    unsafe { OpenClipboard(None) }?;
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }
    let _guard = Guard;
    let handle = unsafe { GetClipboardData(13) }?;
    let memory = windows::Win32::Foundation::HGLOBAL(handle.0);
    let size = unsafe { GlobalSize(memory) };
    if !(2..=65536).contains(&size) {
        return Err("剪贴板文本超出 demo 预算".into());
    }
    let pointer = unsafe { GlobalLock(memory) };
    if pointer.is_null() {
        return Err("剪贴板内存不可读".into());
    }
    let units = unsafe { std::slice::from_raw_parts(pointer.cast::<u16>(), size / 2) };
    let end = units.iter().position(|u| *u == 0).unwrap_or(units.len());
    let text = String::from_utf16_lossy(&units[..end]);
    unsafe {
        let _ = GlobalUnlock(memory);
    }
    Ok(text)
}
