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
    /// 删除光标前的字符。
    Backspace,
    /// 删除光标后的字符或选中对象。
    Delete,
    /// 光标左移。
    ArrowLeft,
    /// 光标右移。
    ArrowRight,
    /// 光标上移。
    ArrowUp,
    /// 光标下移。
    ArrowDown,
    /// 行或文档起点。
    Home,
    /// 行或文档终点。
    End,
    /// 向上翻页。
    PageUp,
    /// 向下翻页。
    PageDown,
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
