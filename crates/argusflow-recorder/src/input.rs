//! 输入采样与已脱敏原始输入 DTO；平台 Hook 数据不会直接序列化。

use argusflow_core::{KeyChord, ScreenPoint, WindowIdentity};
use serde::{Deserialize, Serialize};

/// 物理按下/释放；键盘重复 down 保留其真实次序。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputPhase {
    /// 按下。
    Down,
    /// 释放。
    Up,
}

/// 物理鼠标按键，事件不在录制期间提升为 Click。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    /// 左键。
    Left,
    /// 右键。
    Right,
    /// 中键。
    Middle,
    /// 第一个扩展侧键。
    X1,
    /// 第二个扩展侧键。
    X2,
}

/// Hook 到 worker 的瞬时最小数据，不实现 Debug/Serialize 以避免键码泄露。
#[derive(Clone, Copy)]
pub(crate) enum PhysicalInput {
    /// 顶层窗口显示或成为前台，不推断为启动工作流动作。
    Window {
        window: WindowIdentity,
        change: WindowChange,
    },
    /// 剪贴板内容版本；摄入时核对，避免读取之后的内容冒充原事件。
    Clipboard { sequence_number: u32 },
    /// 物理鼠标按键。
    Mouse {
        /// 虚拟屏幕物理像素点。
        point: ScreenPoint,
        /// 实际鼠标按键。
        button: MouseButton,
        /// 按下/释放阶段。
        phase: InputPhase,
    },
    /// 保留移动路径，后续 AI 可结合按下/释放理解拖拽。
    Move {
        /// 虚拟屏幕物理像素点。
        point: ScreenPoint,
    },
    /// 滚轮保留原始方向与增量。
    Wheel {
        /// 虚拟屏幕物理像素点。
        point: ScreenPoint,
        /// Win32 原始滚轮 delta，通常以 120 为一格。
        delta: i16,
        /// true 表示水平滚轮。
        horizontal: bool,
    },
    /// 原始虚拟键、扫描码与 flags，字符转换只能在 worker 内执行。
    Key {
        /// Win32 virtual-key code，尚未经脱敏。
        virtual_key: u32,
        /// 硬件扫描码，尚未经脱敏。
        scan_code: u32,
        /// LL keyboard flags；注入标记已由 Hook 过滤。
        flags: u32,
        /// 按下/释放阶段。
        phase: InputPhase,
    },
}

/// Hook 有界队列消息；time 是 Win32 事件产生时的 GetTickCount 毫秒低 32 位。
#[derive(Clone, Copy)]
pub(crate) struct PhysicalEvent {
    /// 单次录制从 1 开始的序号；缺口代表 hook queue 丢失事件。
    pub sequence: u64,
    /// Win32 hook 原始事件时钟。
    pub timestamp_ms: u32,
    /// 尚未持久化的最小输入。
    pub input: PhysicalInput,
}

/// 已验证的文本或遮盖标记；遮盖不泄露原始字符串长度。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum RecordedText {
    /// 仅允许在已确认普通字段上保存。
    Plain(String),
    /// 用户后续应将此操作替换为变量输入。
    Redacted,
}

/// Raw Trace 的唯一输入表示；敏感 Key 不保留可逆的 vk/scan code。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RawInput {
    /// 连续移动合并为轨迹事实，保留原始点数、时间、按键状态和关键坐标。
    PointerMotion(crate::PointerMotion),
    /// 原生窗口切换或出现，具体 EXE/标题保存在事件证据中。
    Window {
        /// 事件携带的窗口身份。
        window: WindowIdentity,
        /// 实际观察到的窗口变化。
        change: WindowChange,
    },
    /// 独立剪贴板事件，不把 Ctrl+V 猜成文本输入。
    Clipboard {
        /// Windows 剪贴板版本，用于关联复制/粘贴上下文。
        sequence_number: u32,
        /// 该版本的文本或无法读取的明确状态。
        content: ClipboardContent,
    },
    /// 鼠标按键原样保留。
    Mouse {
        /// 虚拟屏幕物理像素点。
        point: ScreenPoint,
        /// 实际鼠标按键。
        button: MouseButton,
        /// 按下/释放阶段。
        phase: InputPhase,
    },
    /// 移动原始坐标。
    Move {
        /// 虚拟屏幕物理像素点。
        point: ScreenPoint,
    },
    /// 原始滚轮事实。
    Wheel {
        /// 虚拟屏幕物理像素点。
        point: ScreenPoint,
        /// Win32 原始滚轮 delta。
        delta: i16,
        /// true 表示水平滚轮。
        horizontal: bool,
    },
    /// 已检查敏感性的键盘事件。
    Key {
        /// 敏感或未知字段为空。
        virtual_key: Option<u32>,
        /// 敏感或未知字段为空。
        scan_code: Option<u32>,
        /// 原始 LL keyboard flags；敏感/未知字段为空。
        flags: Option<u32>,
        /// 按下或释放。
        phase: InputPhase,
        /// Worker 的布局转换结果；释放事件不重复字符。
        text: Option<RecordedText>,
        /// 观察到的组合键，不在录制阶段编译为 Workflow 操作。
        chord: Option<KeyChord>,
    },
}

/// 顶层窗口生命周期事实，不推断进程是否由用户启动。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowChange {
    /// 窗口成为前台。
    Foreground,
    /// 顶层窗口显示，包括新窗口与重新显示的窗口。
    Appeared,
}

/// 剪贴板观察结果，不将缺失内容伪装为空字符串。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClipboardContent {
    /// 当前版本的 Unicode 文本。
    Text {
        /// 完整内容或有明确截断标记的前缀。
        value: String,
        /// 超过 64K UTF-16 单元时为 true。
        truncated: bool,
    },
    /// 剪贴板没有任何格式。
    Empty,
    /// 当前只有非文本格式，保留变化事件。
    NonText,
    /// 事件到采集间已经再次变化，绝不读取新版本冒充旧内容。
    ChangedBeforeCapture,
    /// 锁定、访问或解码失败。
    Unavailable,
}

/// 只在 worker 内存短期持有的布局解码结果。
#[derive(Default)]
pub(crate) struct DecodedKey {
    /// ToUnicodeEx 解码字符，绝不读取剪贴板。
    pub text: Option<String>,
    /// 现有工作流可执行的组合键。
    pub chord: Option<KeyChord>,
    /// 解码失败原因，不含字符或键码，必须进入诊断。
    pub failure: Option<KeyboardDecodeFailure>,
}

/// 无敏感内容的键盘解码失败分类，避免把输入法问题笼统归为丢字。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardDecodeFailure {
    /// 超出 Win32 键盘状态表的输入。
    InvalidKey,
    /// 当前主键或修饰键组合尚未支持。
    UnsupportedChord,
    /// 无法及时确认输入窗口。
    MissingWindow,
    /// 无法确认该窗口的 GUI 线程。
    MissingThread,
    /// LL Hook 没有输入法最终提交文本，拒绝将拼音冒充提交结果。
    InputMethodActive,
    /// 不修改系统死键状态，因此无法还原组合。
    DeadKey,
    /// 布局没有产生可保存的字符。
    NoCharacter,
}
