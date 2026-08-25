use serde::{Deserialize, Serialize};

/// 一次完整查询替代方案在各层 `any(...)` 中选择的分支序列。
///
/// 路径按查询树的稳定深度优先顺序记录，并使用字典序参与全局 Planner 排序。
/// 空路径表示查询中没有 fallback 分支。
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BranchPath(Vec<usize>);

impl BranchPath {
    /// 创建不包含 `any(...)` 选择的根替代方案。
    pub const fn root() -> Self {
        Self(Vec::new())
    }

    /// 从外到内、按查询树顺序创建一个显式分支路径。
    pub fn from_indices(indices: impl IntoIterator<Item = usize>) -> Self {
        Self(indices.into_iter().collect())
    }

    /// 在已有嵌套选择前添加当前 `any(...)` 的原始分支索引。
    pub fn prepend(&mut self, branch_index: usize) {
        self.0.insert(0, branch_index);
    }

    /// 按查询树顺序连接关系表达式两侧的完整分支选择。
    pub fn append(&mut self, suffix: &Self) {
        self.0.extend_from_slice(&suffix.0);
    }

    /// 返回供 Explain、测试和只读调用方观察的分支索引序列。
    pub fn as_slice(&self) -> &[usize] {
        &self.0
    }
}

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendQueryCapability {
    /// 被分析的后端。
    pub backend: QueryBackend,
    /// 语义支持等级。
    pub level: SupportLevel,
    /// 预计执行成本。
    pub estimated_cost: QueryCost,
    /// 当前计划唯一对应的完整 fallback 分支路径。
    ///
    /// Backend compiler 必须为每条可执行路径分别生成计划，禁止在一个计划内合并
    /// 不连续分支，否则跨后端执行无法保持全局声明顺序。
    pub branch_path: BranchPath,
}
