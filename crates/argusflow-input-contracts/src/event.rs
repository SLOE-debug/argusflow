//! 原始事实与平台来源状态；虚拟键不代表提交文字。
use serde::{Deserialize, Serialize};

/// 本应用注入标记；仅用于标明自身来源，不是认证机制。
pub const OWN_INPUT_TAG: usize = 0x41524755;

/// 输入的已知来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputOrigin {
    /// 系统未标记注入，不保证来自物理硬件。
    System,
    /// Windows 标记注入，但无法确认创建者。
    InjectedUnknown,
    /// 携带本应用约定的注入标记。
    ArgusFlow,
}
/// 物理屏幕坐标，允许负值。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    /// 横坐标。
    pub x: i32,
    /// 纵坐标。
    pub y: i32,
}
/// 鼠标按钮。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Button {
    /// 左键。
    Left,
    /// 右键。
    Right,
    /// 中键。
    Middle,
    /// 侧键，保留系统编号。
    Extra(u16),
}
/// 消息到达时的窗口线索；只在该事件时刻有效，不能用作回放定位器。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowContext {
    /// HWND 的数值线索，不持有窗口。
    pub handle: i64,
    /// 进程 ID，可能复用。
    pub pid: u32,
    /// 前台切换／窗口销毁观察代际，跨此边界不得合并操作。
    pub epoch: u64,
}
/// Hook / WinEvent 实际收到的事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputKind {
    /// 鼠标按下或释放。
    Button {
        /// 按钮。
        button: Button,
        /// 是否按下。
        down: bool,
    },
    /// 未合并的移动事实。
    Move,
    /// 原始滚轮增量，不转换为行数。
    Wheel {
        /// 水平轴。
        horizontal: bool,
        /// Windows 原始有符号增量。
        delta: i16,
    },
    /// 不翻译成字符的键盘消息。
    Key {
        /// 虚拟键。
        vk: u32,
        /// 扫描码。
        scan: u32,
        /// 按下／释放。
        down: bool,
        /// 按下时键已处于按住状态。
        repeat: bool,
        /// 扩展键标记。
        extended: bool,
    },
    /// 前台变化或前台窗口销毁。
    Context,
}
/// 同一监听会话内按接收顺序编号的输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputEvent {
    /// 监听序号，队列故障后不能假装连续。
    pub sequence: u64,
    /// 原始 QPC；由装配边界转换到采样域。
    pub qpc: i64,
    /// 系统消息毫秒 tick，仅作原始来源事实。
    pub system_time: u32,
    /// 该事件的屏幕坐标；键盘事件为当时光标位置。
    pub point: Point,
    /// 前台窗口线索。
    pub window: WindowContext,
    /// 实际前台 HWND，不等同于鼠标命中候选窗口。
    pub foreground_handle: i64,
    /// 来源标记。
    pub origin: InputOrigin,
    /// 系统原始 flags。
    pub flags: u32,
    /// 输入类型。
    pub kind: InputKind,
}
