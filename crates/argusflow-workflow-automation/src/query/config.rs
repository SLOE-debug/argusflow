//! 目标动作共享的保存契约；平台决定范围端口类型，参数通过 Task.inputs 绑定。
use serde::{Deserialize, Serialize};

/// 自动化平台，不代表操作系统；运行时禁止自动切换。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetPlatform {
    /// 已绑定窗口中的桌面控件。
    Uia,
    /// 已连接页面中的网页元素。
    Cdp,
    /// 已由宿主配置采样区域和模型的 OCR 来源。
    Ocr,
}
impl TargetPlatform {
    /// scope 端口要求的资源类型；窗口、页面和 OCR 区域不可混用。
    pub fn scope_type(self) -> &'static str {
        match self {
            Self::Uia => crate::resources::WINDOW,
            Self::Cdp => crate::resources::PAGE,
            Self::Ocr => crate::resources::SOURCE,
        }
    }
}

/// 等待当前目标的条件；变化观察使用独立的观察契约。
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitCondition {
    /// 至少一个匹配。
    Exists,
    /// 成功查询后无匹配。
    Absent,
    /// 恰好一个匹配。
    Unique,
}
impl WaitCondition {
    pub(crate) fn matches(self, count: usize) -> bool {
        match self {
            Self::Exists => count > 0,
            Self::Absent => count == 0,
            Self::Unique => count == 1,
        }
    }
}

/// 查询和点击共用的静态配置。实际范围通过 scope 资源端口绑定。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetConfig {
    /// 明确选择的平台。
    pub platform: TargetPlatform,
    /// 原样保存的中文 AQL；参数值不会插入此字符串。
    pub query: String,
}

/// 等待目标设置；总等待时间仅由 Node.timeout_ms 提供。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WaitTargetConfig {
    /// 明确选择的平台。
    pub platform: TargetPlatform,
    /// 原样保存的中文 AQL。
    pub query: String,
    /// 检查间隔，10..=60000 毫秒。
    pub interval_ms: u64,
    /// 不把查询失败当作目标不存在。
    pub condition: WaitCondition,
}

/// 输入方式必须显式声明，避免覆盖已有草稿。
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEntryMode {
    /// 追加到现有内容之后。
    Append,
    /// 替换全部内容。
    Replace,
    /// 在已确认焦点的当前选区输入。
    Selection,
    /// 只向已聚焦目标的当前选区输入，不执行聚焦或改变光标。
    FocusedSelection,
}

/// 输入文本节点配置，业务文本由 text 输入表达式提供。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypeTargetConfig {
    /// 明确选择的平台。
    pub platform: TargetPlatform,
    /// 原样保存的中文 AQL。
    pub query: String,
    /// 显式输入方式。
    pub mode: TextEntryMode,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeysTargetConfig {
    pub platform: TargetPlatform,
    pub query: String,
    pub keys: String,
}
