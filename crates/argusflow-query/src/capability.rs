use serde::{Deserialize, Serialize};

/// 能够编译 AQL 的后端家族。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryBackend {
    /// Windows UI Automation tree。
    WindowsUia,
    /// Chromium DOM 与 Accessibility tree。
    BrowserCdp,
    /// OCR 与 GUI 元素检测形成的视觉树。
    Vision,
}

/// 后端保持查询语义所需的实现方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    /// 后端原生查询能力可完整表达。
    Native,
    /// 原生查询缩小候选集后由本地 residual filter 完成。
    Hybrid,
    /// 需要额外遍历或多次查询模拟。
    Emulated,
    /// 后端无法保证完整语义。
    Unsupported,
}

impl SupportLevel {
    /// 返回从高到低的路由质量序号。
    pub const fn rank(self) -> u8 {
        match self {
            Self::Native => 0,
            Self::Hybrid => 1,
            Self::Emulated => 2,
            Self::Unsupported => 3,
        }
    }

    /// 判断后端是否能完整执行查询。
    pub const fn is_supported(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// 查询计划的粗粒度成本等级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryCost {
    /// 单次原生查询或等价快路径。
    Low,
    /// 需要缓存属性与本地 residual filter。
    Medium,
    /// 需要树遍历、多分支查询或高成本感知。
    High,
}

impl QueryCost {
    /// 返回从低到高的路由排序序号。
    pub const fn rank(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
        }
    }
}

/// 查询是否只使用跨平台语义契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueryPortability {
    /// 未使用任何 backend namespace 或原生 escape hatch。
    Portable,
    /// 查询显式依赖一个或多个后端家族。
    BackendSpecific {
        /// 查询源码中引用的后端，按稳定枚举顺序排列。
        backends: Vec<QueryBackend>,
    },
}

/// 单个后端对已规范化查询的能力判断。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendQueryCapability {
    /// 被分析的后端。
    pub backend: QueryBackend,
    /// 语义支持等级。
    pub level: SupportLevel,
    /// 预计执行成本。
    pub estimated_cost: QueryCost,
}

/// Analyzer 可稳定暴露给编辑器的警告类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryWarningKind {
    /// 使用了显式后端属性或原生查询。
    BackendSpecificProperty,
    /// 计划需要高成本遍历或多分支模拟。
    ExpensiveTraversal,
    /// 正则必须作为 residual filter 执行。
    RegexResidualFilter,
    /// 查询没有明确单结果选择规则。
    PotentialMultiMatch,
    /// 某个后端无法保证查询语义。
    UnsupportedBackend,
}

/// 一项带可选后端归属的查询分析警告。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryWarning {
    /// 稳定警告类别。
    pub kind: QueryWarningKind,
    /// 仅影响特定后端时记录该后端。
    pub backend: Option<QueryBackend>,
    /// 面向用户的解释。
    pub message: String,
}
