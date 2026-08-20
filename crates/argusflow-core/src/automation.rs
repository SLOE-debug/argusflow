use serde::{Deserialize, Serialize};

/// 可由自动化后端执行的用户操作。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AutomationAction {
    /// 点击指定目标。
    Click {
        /// 要定位并点击的目标。
        target: Selector,
    },
    /// 将文本写入指定目标。
    SetValue {
        /// 要定位并写入的目标。
        target: Selector,
        /// 要设置的文本值。
        value: String,
    },
}

/// 用于定位自动化目标的策略。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Selector {
    /// 通过 Windows UI Automation 属性定位原生控件。
    Native {
        /// 原生控件的可见名称；为空时不参与匹配。
        name: Option<String>,
        /// 原生控件的自动化 ID；为空时不参与匹配。
        automation_id: Option<String>,
        /// 原生控件类型（例如 Button）；为空时不限制类型。
        control_type: Option<String>,
    },
    /// 通过 CSS 选择器定位浏览器 DOM 元素。
    Browser {
        /// 浏览器 DOM 查询使用的 CSS 选择器。
        css: String,
    },
    /// 通过 OCR 或视觉模型匹配屏幕文字。
    VisualText {
        /// 视觉/OCR 后端需要匹配的文字。
        text: String,
        /// 是否要求文字完全相等，而不是允许部分匹配。
        exact: bool,
    },
    /// 直接使用屏幕像素坐标定位目标。
    Coordinate {
        /// 屏幕坐标的水平分量，单位为像素。
        x: i32,
        /// 屏幕坐标的垂直分量，单位为像素。
        y: i32,
    },
}

/// 执行动作时可选的后端类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    /// Windows UI Automation 后端。
    WindowsUia,
    /// 基于浏览器调试协议的后端。
    BrowserCdp,
    /// 视觉缓存匹配后端。
    VisualCache,
    /// 轻量 OCR 后端。
    OcrTiny,
    /// 中等精度 OCR 后端。
    OcrMedium,
    /// 基于 GUI grounding 的后端。
    GuiGrounding,
    /// Windows SendInput 输入后端。
    SendInput,
}

/// 自动化动作成功后的结果摘要。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionOutcome {
    /// 实际处理该动作的后端。
    pub backend: BackendKind,
    /// 面向执行事件消费者的结果说明。
    pub message: String,
}
