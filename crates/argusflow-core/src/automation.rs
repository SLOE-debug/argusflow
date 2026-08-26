use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AqlQuery, CapabilitySet, ResourceRef, ValueExpr};

/// Workflow 层保存的语义界面操作。
///
/// 值表达式和资源作用域由 Runtime 解析；后端只接收解析后的 `AutomationAction`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiOperation {
    /// 点击指定目标。
    Click {
        /// 要定位并点击的目标。
        target: AutomationTarget,
    },
    /// 将解析后的文本写入指定目标。
    SetValue {
        /// 要定位并写入的目标。
        target: AutomationTarget,
        /// 在节点准备阶段解析的文本表达式。
        value: ValueExpr,
    },
    /// 读取元素面向用户的文本。
    GetText {
        /// 要定位并读取的目标。
        target: AutomationTarget,
    },
    /// 读取元素值模式公开的值。
    GetValue {
        /// 要定位并读取的目标。
        target: AutomationTarget,
    },
    /// 批量读取链接元素的可见标题和已解析绝对 URL。
    ///
    /// `text` 输出中每条记录使用制表符分隔标题与 URL，并以 `\r\n` 结尾；
    /// `links` 输出保留结构化数组，供后续数据节点扩展使用。
    CollectLinks {
        /// 要批量定位的链接集合。
        target: AutomationTarget,
    },
}

impl UiOperation {
    /// 返回操作使用的只读目标契约。
    pub const fn target(&self) -> &AutomationTarget {
        match self {
            Self::Click { target }
            | Self::SetValue { target, .. }
            | Self::GetText { target }
            | Self::GetValue { target }
            | Self::CollectLinks { target } => target,
        }
    }
}

/// 已由 Runtime 解析全部表达式、可直接交给自动化后端的动作。
#[derive(Debug, Clone, PartialEq)]
pub enum AutomationAction {
    /// 点击指定目标。
    Click {
        /// 要定位并点击的目标。
        target: AutomationTarget,
    },
    /// 将文本写入指定目标。
    SetValue {
        /// 要定位并写入的目标。
        target: AutomationTarget,
        /// 已冻结的完整文本值。
        value: String,
    },
    /// 读取指定目标面向用户的文本。
    GetText {
        /// 要定位并读取的目标。
        target: AutomationTarget,
    },
    /// 读取指定目标的值。
    GetValue {
        /// 要定位并读取的目标。
        target: AutomationTarget,
    },
    /// 批量读取链接元素的可见标题和已解析绝对 URL。
    CollectLinks {
        /// 要批量定位的链接集合。
        target: AutomationTarget,
    },
}

impl AutomationAction {
    /// 返回动作使用的只读目标契约。
    pub const fn target(&self) -> &AutomationTarget {
        match self {
            Self::Click { target }
            | Self::SetValue { target, .. }
            | Self::GetText { target }
            | Self::GetValue { target }
            | Self::CollectLinks { target } => target,
        }
    }
}

/// 动作目标及其资源作用域、定位语义与后端执行偏好。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationTarget {
    /// 操作相对于当前上下文或某个应用资源执行。
    pub scope: TargetScope,
    /// 描述目标位置的跨后端定位契约。
    pub locator: TargetLocator,
    /// 查询规划器使用的候选集合约束与稳定偏好顺序。
    pub backend_policy: BackendPolicy,
}

impl AutomationTarget {
    /// 创建由 AQL 描述、使用当前上下文并自动选择后端的目标。
    pub fn query(query: AqlQuery) -> Self {
        Self {
            scope: TargetScope::Current,
            locator: TargetLocator::Query { query },
            backend_policy: BackendPolicy::default(),
        }
    }

    /// 创建由屏幕物理像素坐标描述的当前上下文目标。
    pub fn coordinate(x: i32, y: i32) -> Self {
        Self {
            scope: TargetScope::Current,
            locator: TargetLocator::Coordinate {
                point: ScreenPoint { x, y },
            },
            backend_policy: BackendPolicy::default(),
        }
    }
}

/// 语义界面操作使用的逻辑资源作用域。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TargetScope {
    /// 使用执行瞬间的全局前台窗口或浏览器上下文。
    Current,
    /// 使用 Application 节点产生的逻辑会话。
    Application {
        /// 指向应用节点 `session` 资源端口的引用。
        resource: ResourceRef,
    },
    /// 在 Browser 节点创建的隔离 CDP 页面会话内执行。
    Browser {
        /// 指向 Browser 节点 `session` 资源端口的引用。
        resource: ResourceRef,
    },
}

/// 用于定位自动化目标的强类型策略。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TargetLocator {
    /// 通过平台无关 AQL 查询定位语义元素。
    Query {
        /// 持久化的 AQL 源码与独立语言版本。
        query: AqlQuery,
    },
    /// 通过 OCR 或视觉模型描述目标。
    Visual {
        /// 视觉后端使用的查询条件。
        query: VisualQuery,
    },
    /// 直接使用屏幕物理像素坐标定位目标。
    Coordinate {
        /// 目标屏幕点。
        point: ScreenPoint,
    },
}

/// 顶层应用窗口标题的强类型匹配规则。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WindowTitleMatcher {
    /// 窗口标题必须与配置值完全相等，忽略 Unicode 大小写。
    Equal {
        /// 用于匹配窗口标题的非空文本。
        value: String,
    },
    /// 窗口标题必须包含配置值，忽略 Unicode 大小写。
    Contains {
        /// 用于匹配窗口标题的非空片段。
        value: String,
    },
}

impl WindowTitleMatcher {
    /// 返回匹配器携带的只读文本。
    pub fn value(&self) -> &str {
        match self {
            Self::Equal { value } | Self::Contains { value } => value,
        }
    }
}

/// 视觉/OCR 后端使用的显式目标描述。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualQuery {
    /// 需要识别的屏幕文字。
    pub text: String,
    /// 是否要求识别文字完全相等。
    pub exact: bool,
}

/// Windows 虚拟屏幕中的物理像素点。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenPoint {
    /// 水平坐标，单位为物理像素。
    pub x: i32,
    /// 垂直坐标，单位为物理像素。
    pub y: i32,
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

/// 用户对 Planner 候选后端施加的开放集合约束与偏好顺序。
///
/// `allow` 为空表示允许所有已注册后端；`deny` 始终优先；`prefer` 只影响通过过滤后
/// 的候选排序，不会使一个不支持语义或不可用的后端变成可执行候选。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendPolicy {
    /// 允许参与规划的后端集合；空集合表示不限制。
    pub allow: Vec<BackendKind>,
    /// 即使出现在 allow 中也必须排除的后端集合。
    pub deny: Vec<BackendKind>,
    /// 从高到低排列的用户偏好；未列出的后端沿用 Planner 稳定顺序。
    pub prefer: Vec<BackendKind>,
}

impl BackendPolicy {
    /// 创建只允许单个后端参与规划的强制策略。
    pub fn only(backend: BackendKind) -> Self {
        Self {
            allow: vec![backend],
            deny: Vec::new(),
            prefer: vec![backend],
        }
    }

    /// 判断一个已注册后端能否参与候选准备。
    pub fn allows(&self, backend: BackendKind) -> bool {
        (self.allow.is_empty() || self.allow.contains(&backend)) && !self.deny.contains(&backend)
    }

    /// 返回用户偏好序号；未显式列出的后端排在全部用户偏好之后。
    pub fn preference_rank(&self, backend: BackendKind) -> usize {
        self.prefer
            .iter()
            .position(|candidate| *candidate == backend)
            .unwrap_or(self.prefer.len())
    }
}

/// 自动化动作成功后的结构化结果。
#[derive(Debug, Clone, PartialEq)]
pub struct ActionOutcome {
    /// 实际处理该动作的后端。
    pub backend: BackendKind,
    /// 面向执行事件消费者的结果说明。
    pub message: String,
    /// 由读取动作产生的结构化值输出；点击和写入动作返回空映射。
    pub outputs: BTreeMap<String, Value>,
    /// 本次成功动作之前由失败候选产生、已持久化的 evidence 引用。
    pub diagnostic_evidence: Vec<DiagnosticEvidenceReference>,
}

/// 可安全进入 ExecutionEvent 的 Failure Evidence 小型引用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticEvidenceReference {
    /// artifact sink 生成的稳定 evidence 标识。
    pub evidence_id: uuid::Uuid,
    /// 产生失败证据的后端。
    pub backend: BackendKind,
    /// 失败 candidate 的完整 AQL fallback 路径。
    pub branch_path: Vec<usize>,
    /// 是否由更晚 candidate 恢复成功。
    pub recovered_by_fallback: bool,
}

/// Runtime 解析资源引用后传给 ActionDispatcher 的瞬时执行作用域。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationExecutionScope {
    /// 沿用宿主捕获的当前执行上下文。
    Current,
    /// 在已经获取且验证过的应用顶层窗口内执行。
    Window {
        /// 原生 HWND 的无符号不透明表示。
        handle: u64,
        /// 窗口所属进程 ID，用于检测句柄复用。
        process_id: u32,
        /// 应用资源提供器在获取阶段确认的后端能力事实。
        capabilities: CapabilitySet,
    },
    /// 在已获取且仍附加的浏览器页面会话内执行。
    Browser {
        /// 单次运行内浏览器资源的稳定 ID。
        session_id: crate::ResourceId,
        /// 获取阶段冻结的 CDP page target ID。
        target_id: String,
    },
}
