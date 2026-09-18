//! 将用户明确指定的组合键转换为共享输入能力的按键类型。
use argusflow_core::Key;
pub(super) fn parse(source: &str) -> Result<Vec<Key>, String> {
    let mut keys = Vec::new();
    for value in source.split('+') {
        let key = match value {
            "Control" => Key::Control,
            "Shift" => Key::Shift,
            "Alt" => Key::Alt,
            "Enter" => Key::Enter,
            "Escape" => Key::Escape,
            "Tab" => Key::Tab,
            "Home" => Key::Home,
            "End" => Key::End,
            "Backspace" => Key::Backspace,
            "Delete" => Key::Delete,
            "Space" => Key::Space,
            "Left" => Key::Left,
            "Right" => Key::Right,
            "Up" => Key::Up,
            "Down" => Key::Down,
            s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphabetic() => {
                Key::Letter(char::from(s.as_bytes()[0]).to_ascii_uppercase())
            }
            _ => return Err(format!("不支持的按键：{value}")),
        };
        if keys.contains(&key) {
            return Err("组合键不能重复".into());
        }
        keys.push(key);
    }
    if keys.len() > 8 {
        return Err("组合键必须为 1–8 个按键".into());
    }
    Ok(keys)
}
