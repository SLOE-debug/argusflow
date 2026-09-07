//! 非 Hook ingestion 线程的布局解码；不读取剪贴板、不读取输入框值。

use crate::{InputPhase, KeyboardDecodeFailure, input::DecodedKey};

#[cfg(test)]
#[path = "tests/keyboard.rs"]
mod tests;
use argusflow_core::{InspectionContext, KeyChord, KeyboardKey, KeyboardModifier};
use windows::Win32::{
    Foundation::HWND,
    UI::{
        Input::{
            Ime::{ImmGetContext, ImmGetOpenStatus, ImmReleaseContext},
            KeyboardAndMouse::{GetAsyncKeyState, GetKeyState, GetKeyboardLayout, ToUnicodeEx},
        },
        WindowsAndMessaging::{GUITHREADINFO, GetGUIThreadInfo, GetWindowThreadProcessId},
    },
};

/// 录制队列真实 down/up 驱动的键盘状态；不依赖 Hook 尚未更新的 GetAsyncKeyState。
pub(crate) struct KeyboardDecoder {
    /// 每个虚拟键的 high-bit pressed 与 low-bit toggle。
    state: [u8; 256],
    /// 无副作用 ToUnicodeEx 不能提交死键状态，后续组合明确标记不支持。
    dead_key: bool,
}

impl KeyboardDecoder {
    pub(crate) fn new() -> Self {
        let mut state = [0; 256];
        for (key, value) in state.iter_mut().enumerate() {
            // SAFETY: 只读取起始键盘状态，不注入输入。
            *value = if unsafe { GetAsyncKeyState(key as i32) } < 0 {
                0x80
            } else {
                0
            };
        }
        for key in [0x14, 0x90, 0x91] {
            state[key] |= (unsafe { GetKeyState(key as i32) } & 1) as u8;
        }
        Self {
            state,
            dead_key: false,
        }
    }

    /// 在捕获时刻使用目标窗口线程的布局翻译键码；参数和临时字符不被日志记录。
    pub(crate) fn decode(
        &mut self,
        virtual_key: u32,
        scan_code: u32,
        phase: InputPhase,
        context: Option<&InspectionContext>,
    ) -> DecodedKey {
        let Ok(key) = usize::try_from(virtual_key) else {
            return DecodedKey {
                failure: Some(KeyboardDecodeFailure::InvalidKey),
                ..Default::default()
            };
        };
        if key >= self.state.len() {
            return DecodedKey {
                failure: Some(KeyboardDecodeFailure::InvalidKey),
                ..Default::default()
            };
        }
        let down = phase == InputPhase::Down;
        if down && self.state[key] & 0x80 == 0 && matches!(key, 0x14 | 0x90 | 0x91) {
            self.state[key] ^= 1;
        }
        self.state[key] = (self.state[key] & 1) | if down { 0x80 } else { 0 };
        for (generic, left, right) in [(0x10, 0xa0, 0xa1), (0x11, 0xa2, 0xa3), (0x12, 0xa4, 0xa5)] {
            if key == left || key == right {
                self.state[generic] = (self.state[left] | self.state[right]) & 0x80;
            }
        }
        if !down || matches!(key, 0x10..=0x12 | 0xa0..=0xa5 | 0x5b | 0x5c) {
            return DecodedKey::default();
        }
        let control = self.state[0x11] & 0x80 != 0;
        let alt = self.state[0x12] & 0x80 != 0;
        let shift = self.state[0x10] & 0x80 != 0;
        // 右 Alt 在 AltGr 布局中是字符输入，不能误归为 Ctrl+Alt 快捷键。
        let alt_gr = control && self.state[0xa5] & 0x80 != 0;
        let key = match virtual_key {
            0x0d => Some(KeyboardKey::Enter),
            0x1b => Some(KeyboardKey::Escape),
            0x09 => Some(KeyboardKey::Tab),
            0x08 => Some(KeyboardKey::Backspace),
            0x2e => Some(KeyboardKey::Delete),
            0x25 => Some(KeyboardKey::ArrowLeft),
            0x27 => Some(KeyboardKey::ArrowRight),
            0x26 => Some(KeyboardKey::ArrowUp),
            0x28 => Some(KeyboardKey::ArrowDown),
            0x24 => Some(KeyboardKey::Home),
            0x23 => Some(KeyboardKey::End),
            0x21 => Some(KeyboardKey::PageUp),
            0x22 => Some(KeyboardKey::PageDown),
            0x30..=0x39 | 0x41..=0x5a if (control || alt) && !alt_gr => char::from_u32(virtual_key)
                .map(|value| KeyboardKey::Character {
                    value: value.to_string(),
                }),
            _ => None,
        };
        if let Some(key) = key {
            let mut modifiers = Vec::new();
            if control {
                modifiers.push(KeyboardModifier::Control);
            }
            if alt {
                modifiers.push(KeyboardModifier::Alt);
            }
            if shift {
                modifiers.push(KeyboardModifier::Shift);
            }
            return DecodedKey {
                chord: Some(KeyChord { key, modifiers }),
                ..Default::default()
            };
        }
        if (control || alt) && !alt_gr
            || self.state[0x5b] & 0x80 != 0
            || self.state[0x5c] & 0x80 != 0
        {
            return DecodedKey {
                failure: Some(KeyboardDecodeFailure::UnsupportedChord),
                ..Default::default()
            };
        }
        let Some(context) = context else {
            return DecodedKey {
                failure: Some(KeyboardDecodeFailure::MissingWindow),
                ..Default::default()
            };
        };
        let window = HWND(context.window.handle as *mut _);
        // SAFETY: 借用窗口句柄只用于线程与布局查询，输出无平台对象。
        let thread_id = unsafe { GetWindowThreadProcessId(window, None) };
        if thread_id == 0 {
            return DecodedKey {
                failure: Some(KeyboardDecodeFailure::MissingThread),
                ..Default::default()
            };
        }
        let input_method = if virtual_key == 0xe5 {
            Ok(true)
        } else {
            ime_open(thread_id)
        };
        if input_method != Ok(false) {
            return DecodedKey {
                failure: Some(
                    input_method
                        .err()
                        .unwrap_or(KeyboardDecodeFailure::InputMethodActive),
                ),
                ..Default::default()
            };
        }
        let layout = unsafe { GetKeyboardLayout(thread_id) };
        let mut buffer = [0_u16; 16];
        // SAFETY: 256-byte keyboard state 与 UTF-16 输出有效；flags=4 禁止修改目标的死键状态。
        let count = unsafe {
            ToUnicodeEx(
                virtual_key,
                scan_code,
                &self.state,
                &mut buffer,
                4,
                Some(layout),
            )
        };
        if count < 0 || self.dead_key {
            self.dead_key = count < 0;
            return DecodedKey {
                failure: Some(KeyboardDecodeFailure::DeadKey),
                ..Default::default()
            };
        }
        let text = if count > 0 {
            String::from_utf16(&buffer[..(count as usize).min(buffer.len())])
                .ok()
                .filter(|text| !text.chars().any(char::is_control))
        } else {
            None
        };
        DecodedKey {
            failure: text.is_none().then_some(KeyboardDecodeFailure::NoCharacter),
            text,
            chord: None,
        }
    }
}

/// LL Hook 不携带 IME composition/commit；启用时拒绝把拼音键序列冒充已提交文本。
fn ime_open(thread_id: u32) -> Result<bool, KeyboardDecodeFailure> {
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: 查询 GUI focus 的同步输出，并在同一线程归还借用 IME context。
    if unsafe { GetGUIThreadInfo(thread_id, &mut info) }.is_err() {
        return Err(KeyboardDecodeFailure::MissingThread);
    }
    let context = unsafe { ImmGetContext(info.hwndFocus) };
    if context.0.is_null() {
        return Ok(false);
    }
    let open = unsafe { ImmGetOpenStatus(context) }.as_bool();
    unsafe {
        let _ = ImmReleaseContext(info.hwndFocus, context);
    }
    Ok(open)
}
