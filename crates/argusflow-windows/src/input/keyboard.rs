//! Unicode 与组合键序列构造，保持按下/释放成对。
use super::inject::{ensure_released, invalid};
use crate::WindowsError as Failure;
use argusflow_core::Key;
use std::collections::HashSet;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub(super) fn text(text: &str) -> Result<(Vec<INPUT>, Vec<INPUT>), Failure> {
    if text.is_empty() || text.len() > 16_384 {
        return Err(invalid("文本必须非空且不超过 16 KiB"));
    }
    for key in [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN] {
        ensure_released(key)?;
    }
    let mut events = Vec::new();
    let mut releases = Vec::new();
    for unit in text.encode_utf16() {
        events.push(event(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE));
        let release = event(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP);
        events.push(release);
        releases.push(release);
    }
    Ok((events, releases))
}

pub(super) fn chord(keys: &[Key]) -> Result<(Vec<INPUT>, Vec<INPUT>), Failure> {
    if keys.is_empty() || keys.len() > 8 {
        return Err(invalid("组合键数量必须为 1-8"));
    }
    let mut seen = HashSet::new();
    let mut events = Vec::new();
    let mut releases = Vec::new();
    for key in keys {
        let (virtual_key, extended) = native_key(*key)?;
        if !seen.insert(virtual_key.0) {
            return Err(invalid("组合键包含重复按键"));
        }
        ensure_released(virtual_key)?;
        let flags = if extended {
            KEYEVENTF_EXTENDEDKEY
        } else {
            KEYBD_EVENT_FLAGS(0)
        };
        events.push(event(virtual_key, 0, flags));
        releases.push(event(virtual_key, 0, flags | KEYEVENTF_KEYUP));
    }
    releases.reverse();
    events.extend_from_slice(&releases);
    Ok((events, releases))
}

fn event(key: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: scan,
                dwFlags: flags,
                ..Default::default()
            },
        },
    }
}

fn native_key(key: Key) -> Result<(VIRTUAL_KEY, bool), Failure> {
    Ok(match key {
        Key::Control => (VK_CONTROL, false),
        Key::Shift => (VK_SHIFT, false),
        Key::Alt => (VK_MENU, false),
        Key::Meta => (VK_LWIN, true),
        Key::Enter => (VK_RETURN, false),
        Key::Tab => (VK_TAB, false),
        Key::Escape => (VK_ESCAPE, false),
        Key::Backspace => (VK_BACK, false),
        Key::Space => (VK_SPACE, false),
        Key::Delete => (VK_DELETE, true),
        Key::Home => (VK_HOME, true),
        Key::End => (VK_END, true),
        Key::PageUp => (VK_PRIOR, true),
        Key::PageDown => (VK_NEXT, true),
        Key::Left => (VK_LEFT, true),
        Key::Right => (VK_RIGHT, true),
        Key::Up => (VK_UP, true),
        Key::Down => (VK_DOWN, true),
        Key::Letter(letter) if letter.is_ascii_alphabetic() => {
            (VIRTUAL_KEY(letter.to_ascii_uppercase() as u16), false)
        }
        Key::Function(number) if (1..=12).contains(&number) => {
            (VIRTUAL_KEY(VK_F1.0 + u16::from(number - 1)), false)
        }
        Key::Letter(_) | Key::Function(_) => return Err(invalid("字母键或功能键超出范围")),
    })
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/input/keyboard.rs"]
mod tests;
