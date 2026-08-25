use serde::{Deserialize, Serialize};

use crate::AqlQuery;

/// 可由自动化后端执行的用户操作。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
        /// 要设置的文本值。
        value: String,
    },
}

impl AutomationAction {
    /// 返回动作使用的只读目标契约。
    pub const fn target(&self) -> &AutomationTarget {
        match self {
            Self::Click { target } | Self::SetValue { target, .. } => target,
        }
    }
}

/// 动作目标及其独立于查询语义的后端执行偏好。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationTarget {
    /// 描述目标位置的跨后端定位契约。
    pub locator: TargetLocator,
    /// 查询规划器选择后端时使用的提示；不会改变 AQL 本身的语义。
    pub backend_preference: BackendPreference,
}

impl AutomationTarget {
    /// 创建由 AQL 描述、默认自动选择后端的目标。
    pub fn query(query: AqlQuery) -> Self {
        Self {
            locator: TargetLocator::Query { query },
            backend_preference: BackendPreference::Auto,
        }
    }

    /// 创建由屏幕物理像素坐标描述的目标。
    pub const fn coordinate(x: i32, y: i32) -> Self {
        Self {
            locator: TargetLocator::Coordinate {
                point: ScreenPoint { x, y },
            },
            backend_preference: BackendPreference::Auto,
        }
    }
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
    /// 确保指定 Windows 应用可交互后，在其顶层窗口内执行 AQL。
    ApplicationQuery {
        /// 用于复用现有进程或显式启动进程的应用契约。
        application: ApplicationTarget,
        /// 在已确定应用窗口内部执行的 AQL。
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

/// UIA 动作执行前需要定位、恢复或启动的 Windows 应用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationTarget {
    /// 进程身份与启动命令共同使用的绝对 EXE 路径。
    pub executable_path: String,
    /// 直接传给 EXE 的参数列表，不经过 shell 解析。
    pub arguments: Vec<String>,
    /// 从同一 EXE 的顶层窗口中筛选唯一目标的标题规则。
    pub window_title: WindowTitleMatcher,
    /// 启动后等待可交互顶层窗口的最长毫秒数。
    pub launch_timeout_ms: u64,
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

/// 用户对语义查询执行后端的显式偏好。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendPreference {
    /// 根据查询能力和运行上下文自动选择后端。
    #[default]
    Auto,
    /// 强制使用 Windows UI Automation。
    WindowsUia,
    /// 强制使用 Chrome DevTools Protocol。
    BrowserCdp,
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
