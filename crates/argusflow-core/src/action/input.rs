//! UIA 原生输入与 CDP 输入共用的有限参数集合。

/// 鼠标按键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// 左键。
    Left,
    /// 右键。
    Right,
}

/// 单次原子点击序列包含的次数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickCount {
    /// 单击。
    Single,
    /// 双击。
    Double,
}

/// 滚动方向的轴；实际单位由对应能力接口明确说明。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    /// 水平方向。
    Horizontal,
    /// 垂直方向。
    Vertical,
}

/// 组合键中的按键；普通文字使用文本输入接口。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// Control 修饰键。
    Control,
    /// Shift 修饰键。
    Shift,
    /// Alt 修饰键。
    Alt,
    /// Windows/Meta 修饰键。
    Meta,
    /// 回车键。
    Enter,
    /// 制表键。
    Tab,
    /// Escape 键。
    Escape,
    /// 退格键。
    Backspace,
    /// Delete 键。
    Delete,
    /// 空格键。
    Space,
    /// Home 键。
    Home,
    /// End 键。
    End,
    /// 向上翻页。
    PageUp,
    /// 向下翻页。
    PageDown,
    /// 左方向键。
    Left,
    /// 右方向键。
    Right,
    /// 上方向键。
    Up,
    /// 下方向键。
    Down,
    /// ASCII 字母键，调用时校验为 A-Z/a-z。
    Letter(char),
    /// 功能键，调用时校验为 1-12。
    Function(u8),
}
