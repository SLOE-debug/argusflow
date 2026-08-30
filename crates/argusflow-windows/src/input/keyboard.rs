use std::{collections::BTreeSet, mem::size_of};

use argusflow_agent::WindowContext;
use argusflow_core::{KeyChord, KeyboardKey, KeyboardModifier};
use thiserror::Error;
use windows::Win32::{
    Foundation::HWND,
    UI::{
        Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
            KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_CONTROL, VK_ESCAPE, VK_MENU, VK_RETURN,
            VK_SHIFT, VK_TAB,
        },
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
        },
    },
};

/// SendInput 前置窗口校验和事件注入失败。
#[derive(Debug, Error)]
pub(super) enum KeyboardInputError {
    /// AppSession 中的窗口句柄已经失效。
    #[error("目标窗口已经失效")]
    InvalidWindow,
    /// HWND 已被系统复用到另一个进程。
    #[error("目标窗口所属进程已经变化")]
    ProcessMismatch,
    /// Windows 前台锁拒绝了物理输入所需的激活。
    #[error("无法把目标窗口置于前台，已取消输入")]
    ActivationFailed,
    /// 组合键只接受不会依赖输入法布局的 ASCII 字母或数字。
    #[error("组合键字符必须是单个 ASCII 字母或数字")]
    InvalidChordCharacter,
    /// 同一个修饰键不能重复按下。
    #[error("组合键包含重复修饰键")]
    DuplicateModifier,
    /// UIPI 或系统输入队列只接受了部分事件。
    #[error("Windows 只注入了 {inserted}/{requested} 个键盘事件")]
    PartialInjection { inserted: u32, requested: usize },
}

/// 验证窗口身份和前台状态后注入一次组合键。
pub(super) fn inject_chord(
    window: &WindowContext,
    chord: &KeyChord,
) -> Result<(), KeyboardInputError> {
    ensure_foreground_window(window)?;
    let inputs = chord_inputs(chord)?;
    inject_inputs(&inputs)
}

/// 验证窗口身份和前台状态后按 UTF-16 code unit 注入 Unicode 文本。
pub(super) fn inject_text(window: &WindowContext, value: &str) -> Result<(), KeyboardInputError> {
    ensure_foreground_window(window)?;
    let inputs = value
        .encode_utf16()
        .flat_map(|unit| {
            [
                unicode_input(unit, KEYBD_EVENT_FLAGS::default()),
                unicode_input(unit, KEYEVENTF_KEYUP),
            ]
        })
        .collect::<Vec<_>>();
    inject_inputs(&inputs)
}

/// 复验 HWND/PID 并确保输入不会误发到其它前台窗口。
pub(super) fn ensure_foreground_window(window: &WindowContext) -> Result<(), KeyboardInputError> {
    let target = HWND(window.handle as usize as *mut std::ffi::c_void);
    // SAFETY: HWND 只作为不透明系统身份读取，不解引用调用方内存。
    if !unsafe { IsWindow(Some(target)) }.as_bool() {
        return Err(KeyboardInputError::InvalidWindow);
    }
    let mut process_id = 0_u32;
    // SAFETY: process_id 在同步调用期间有效且独占。
    unsafe { GetWindowThreadProcessId(target, Some(&mut process_id)) };
    if process_id != window.process_id {
        return Err(KeyboardInputError::ProcessMismatch);
    }
    // SAFETY: SetForegroundWindow 只尝试激活已复验的目标 HWND。
    if unsafe { GetForegroundWindow() } != target
        && !unsafe { SetForegroundWindow(target) }.as_bool()
    {
        return Err(KeyboardInputError::ActivationFailed);
    }
    // 前台锁可能报告请求已接受但仍把焦点保留在其它窗口，必须再次核验。
    if unsafe { GetForegroundWindow() } != target {
        return Err(KeyboardInputError::ActivationFailed);
    }
    Ok(())
}

/// 以固定修饰键顺序构造 key-down/key-up 事件，避免工作流数组顺序改变结果。
fn chord_inputs(chord: &KeyChord) -> Result<Vec<INPUT>, KeyboardInputError> {
    let unique = chord.modifiers.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != chord.modifiers.len() {
        return Err(KeyboardInputError::DuplicateModifier);
    }
    let modifiers = [
        KeyboardModifier::Control,
        KeyboardModifier::Alt,
        KeyboardModifier::Shift,
    ]
    .into_iter()
    .filter(|modifier| unique.contains(modifier))
    .map(modifier_virtual_key)
    .collect::<Vec<_>>();
    let key = key_virtual_key(&chord.key)?;
    let mut inputs = Vec::with_capacity(modifiers.len() * 2 + 2);
    inputs.extend(
        modifiers
            .iter()
            .copied()
            .map(|modifier| virtual_key_input(modifier, KEYBD_EVENT_FLAGS::default())),
    );
    inputs.push(virtual_key_input(key, KEYBD_EVENT_FLAGS::default()));
    inputs.push(virtual_key_input(key, KEYEVENTF_KEYUP));
    inputs.extend(
        modifiers
            .iter()
            .rev()
            .copied()
            .map(|modifier| virtual_key_input(modifier, KEYEVENTF_KEYUP)),
    );
    Ok(inputs)
}

/// 把封闭修饰键集合映射为 Win32 virtual-key。
const fn modifier_virtual_key(modifier: KeyboardModifier) -> VIRTUAL_KEY {
    match modifier {
        KeyboardModifier::Control => VK_CONTROL,
        KeyboardModifier::Alt => VK_MENU,
        KeyboardModifier::Shift => VK_SHIFT,
    }
}

/// 把布局无关的工作流按键映射为 Win32 virtual-key。
fn key_virtual_key(key: &KeyboardKey) -> Result<VIRTUAL_KEY, KeyboardInputError> {
    match key {
        KeyboardKey::Enter => Ok(VK_RETURN),
        KeyboardKey::Escape => Ok(VK_ESCAPE),
        KeyboardKey::Tab => Ok(VK_TAB),
        KeyboardKey::Character { value }
            if value.len() == 1 && value.as_bytes()[0].is_ascii_alphanumeric() =>
        {
            Ok(VIRTUAL_KEY(value.as_bytes()[0].to_ascii_uppercase() as u16))
        }
        KeyboardKey::Character { .. } => Err(KeyboardInputError::InvalidChordCharacter),
    }
}

/// 创建一个 virtual-key 键盘事件。
fn virtual_key_input(key: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: flags,
                ..KEYBDINPUT::default()
            },
        },
    }
}

/// 创建一个 UTF-16 code unit 键盘事件。
fn unicode_input(unit: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE | flags,
                ..KEYBDINPUT::default()
            },
        },
    }
}

/// 一次性提交完整事件序列，拒绝把部分注入误报为成功。
fn inject_inputs(inputs: &[INPUT]) -> Result<(), KeyboardInputError> {
    if inputs.is_empty() {
        return Ok(());
    }
    // SAFETY: INPUT slice 在同步调用期间有效，结构尺寸来自相同 windows crate 类型。
    let inserted = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if inserted as usize != inputs.len() {
        return Err(KeyboardInputError::PartialInjection {
            inserted,
            requested: inputs.len(),
        });
    }
    Ok(())
}
