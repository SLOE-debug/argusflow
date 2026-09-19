//! 页面选区的纯数据契约，DOM 身份仅在本次文档代际有效。
use serde::{Deserialize, Serialize};

/// 浏览器实际派发的事件类别，不预先解释成业务动作。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageEventKind {
    /// 点击。
    Click,
    /// 右键菜单。
    Contextmenu,
    /// 输入值改变。
    Input,
    /// 控件提交更改。
    Change,
    /// 输入法组合完成。
    Compositionend,
    /// 表单提交。
    Submit,
    /// 指针按下。
    Pointerdown,
    /// 指针释放。
    Pointerup,
    /// 指针手势被取消。
    Pointercancel,
    /// 实际选区变化。
    Selectionchange,
    /// 复制事件，不宣称剪贴板已写入。
    Copy,
    /// 剪切事件。
    Cut,
    /// 粘贴事件，不宣称编辑已经完成。
    Paste,
}
/// 文档节点端点；offset 在文本节点中为 UTF-16，在元素中为子节点索引。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageTextEndpoint {
    /// 会话文档内节点身份。
    pub node: u64,
    /// DOM nodeType，区分文本与元素偏移语义。
    pub node_type: u16,
    /// 原始端点偏移。
    pub offset: u32,
    /// 节点文字前缀，供文本模型解释端点上下文，不用于反查身份。
    pub text: String,
    /// 最近父元素的有界属性。
    pub parent: Option<Box<super::observation::PageTarget>>,
}
/// 浏览器提供的真实选区，不推断第几行或用户意图。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PageSelection {
    /// 活动密码字段，省略所有选区内容。
    Sensitive,
    /// 控件没有暴露文本选区能力。
    Unsupported,
    /// DOM 变化或提供者读取异常。
    Unavailable,
    /// input/textarea 的原生范围。
    Control {
        /// 文档内控件身份。
        node: u64,
        /// UTF-16 起点。
        start: u32,
        /// UTF-16 排他终点。
        end: u32,
        /// 浏览器报告的方向。
        direction: String,
        /// 选中文字前缀。
        text: String,
        /// 是否截断。
        truncated: bool,
    },
    /// 普通 DOM 文本范围，支持跨节点；空选区的端点为空。
    Document {
        /// 原始锚点，保留反向拖选。
        anchor: Option<PageTextEndpoint>,
        /// 原始焦点。
        focus: Option<PageTextEndpoint>,
        /// 是否退化为空光标。
        collapsed: bool,
        /// 实际范围数量。
        range_count: u32,
        /// 选中文字前缀。
        text: String,
        /// 可见区域矩形 [left, top, right, bottom]，视口 CSS 坐标。
        rects: Vec<[f64; 4]>,
        /// 文本或矩形达到预算。
        truncated: bool,
    },
}
