use serde::{Deserialize, Serialize};

/// 组合键中可独立按下和释放的修饰键。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardModifier {
    /// Control 修饰键。
    Control,
    /// Alt 修饰键。
    Alt,
    /// Shift 修饰键。
    Shift,
}

/// 不依赖当前键盘布局的有限按键集合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeyboardKey {
    /// 回车键。
    Enter,
    /// Escape 键。
    Escape,
    /// Tab 键。
    Tab,
    /// 单个 ASCII 字母或数字；主要用于带修饰键的应用快捷键。
    Character {
        /// 必须是单个 ASCII 字母或数字。
        value: String,
    },
}

/// 一次同时按下修饰键和主键、随后逆序释放的键盘输入。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyChord {
    /// 主键。
    pub key: KeyboardKey,
    /// 修饰键集合；执行前会拒绝重复项。
    pub modifiers: Vec<KeyboardModifier>,
}
