//! AQL 的平台无关查询契约。
//!
//! 本模块只表达语义，不包含文本解析、能力分析或具体后端执行计划。

use std::{collections::BTreeMap, fmt, num::NonZeroUsize};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

mod spatial;

pub use spatial::{DistanceMetric, SpatialAnchor, SpatialDirection, ViewportCorner, ViewportEdge};

/// 当前稳定 AQL 语言版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryLanguageVersion {
    /// AQL v1 语法与语义。
    V1,
    /// AQL v2 参数绑定与空间查询语义。
    V2,
}

impl QueryLanguageVersion {
    /// 返回持久化协议使用的数值版本。
    pub const fn number(self) -> u16 {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
        }
    }
}

impl Serialize for QueryLanguageVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(self.number())
    }
}

impl<'de> Deserialize<'de> for QueryLanguageVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match u16::deserialize(deserializer)? {
            1 => Ok(Self::V1),
            2 => Ok(Self::V2),
            version => Err(de::Error::custom(format_args!(
                "unsupported AQL language version {version}"
            ))),
        }
    }
}

/// 工作流中持久化的 AQL 源码；编译缓存不属于该事实来源。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AqlQuery {
    /// 与工作流 schema 独立演进的查询语言版本。
    pub language_version: QueryLanguageVersion,
    /// 用户可读、可编辑的 AQL 文本。
    pub source: String,
    /// 运行时解析的参数表达式；源码只保存 `$name`，不拼接动态值。
    #[serde(default)]
    pub bindings: BTreeMap<String, crate::ValueExpr>,
}

impl AqlQuery {
    /// 创建使用 AQL v1 的持久化查询。
    pub fn v1(source: impl Into<String>) -> Self {
        Self {
            language_version: QueryLanguageVersion::V1,
            source: source.into(),
            bindings: BTreeMap::new(),
        }
    }

    /// 创建使用 AQL v2 的参数化查询。
    pub fn v2(source: impl Into<String>, bindings: BTreeMap<String, crate::ValueExpr>) -> Self {
        Self {
            language_version: QueryLanguageVersion::V2,
            source: source.into(),
            bindings,
        }
    }
}

/// 已解析且通过语义校验的 AQL 查询。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiQuery {
    /// 查询的强类型表达式树。
    pub expression: QueryExpr,
}

impl UiQuery {
    /// 从已校验的根表达式创建查询。
    pub const fn new(expression: QueryExpr) -> Self {
        Self { expression }
    }
}

/// AQL v1 查询代数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueryExpr {
    /// 匹配一个具有指定角色和属性的元素。
    Match {
        /// 元素匹配条件。
        matcher: ElementMatcher,
    },
    /// 在祖先的任意深度后代中查找目标。
    Descendant {
        /// 限定搜索子树的祖先查询。
        ancestor: Box<QueryExpr>,
        /// 在祖先子树内执行的目标查询。
        target: Box<QueryExpr>,
    },
    /// 只在父元素的直接子元素中查找目标。
    Child {
        /// 限定直接子元素范围的父查询。
        parent: Box<QueryExpr>,
        /// 在直接子元素中执行的目标查询。
        target: Box<QueryExpr>,
    },
    /// 按声明顺序组合多个可替代查询。
    Any {
        /// 至少包含两个分支；顺序代表回退优先级。
        queries: Vec<QueryExpr>,
    },
    /// 对内部查询结果取反。
    Not {
        /// 被排除的查询。
        query: Box<QueryExpr>,
    },
    /// 明确选择查询结果中的第一个元素。
    First {
        /// 可能返回多个元素的内部查询。
        query: Box<QueryExpr>,
    },
    /// 明确选择查询结果中从一开始计数的第 N 个元素。
    Nth {
        /// 可能返回多个元素的内部查询。
        query: Box<QueryExpr>,
        /// 从一开始计数且必须非零的索引。
        index: NonZeroUsize,
    },
    /// 以唯一 anchor 为中心按相对方向和归一化距离选择显式名次。
    Nearest {
        /// 元素查询、viewport 角或 viewport 边组成的空间锚点。
        anchor: SpatialAnchor,
        /// 参与方向过滤和距离排序的目标查询。
        target: Box<QueryExpr>,
        /// 相对于锚点的方向约束。
        direction: SpatialDirection,
        /// 按距离 rank 从一开始计数的显式名次。
        index: NonZeroUsize,
        /// 与物理分辨率无关的距离度量。
        metric: DistanceMetric,
    },
    /// 只允许 Browser/CDP 后端执行的原生 CSS 查询。
    Css {
        /// 不由 AQL 解释的完整 CSS selector。
        selector: String,
    },
}

/// 单个 UI 元素的语义角色和属性谓词。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementMatcher {
    /// 元素的跨平台语义角色。
    pub role: ElementRole,
    /// 同一 matcher 内全部按 AND 关系计算的谓词。
    pub predicates: Vec<PropertyPredicate>,
}

/// AQL v1 支持的有限语义角色集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementRole {
    /// 顶层应用窗口。
    Window,
    /// 模态或非模态对话框。
    Dialog,
    /// 通用容器面板。
    Pane,
    /// 可调用按钮。
    Button,
    /// 可编辑文本框。
    TextBox,
    /// 复选框。
    CheckBox,
    /// 单选按钮。
    Radio,
    /// 组合框。
    ComboBox,
    /// 列表容器。
    List,
    /// 列表项。
    ListItem,
    /// 树容器。
    Tree,
    /// 树节点。
    TreeItem,
    /// 选项卡容器。
    Tab,
    /// 选项卡项。
    TabItem,
    /// 菜单容器。
    Menu,
    /// 菜单项。
    MenuItem,
    /// 可导航链接。
    Link,
    /// 图像元素。
    Image,
    /// 表格容器。
    Table,
    /// 表格行。
    Row,
    /// 表格单元格。
    Cell,
    /// 文档根或文档区域。
    Document,
    /// 静态文本。
    Text,
}

impl fmt::Display for ElementRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Window => "window",
            Self::Dialog => "dialog",
            Self::Pane => "pane",
            Self::Button => "button",
            Self::TextBox => "textbox",
            Self::CheckBox => "checkbox",
            Self::Radio => "radio",
            Self::ComboBox => "combobox",
            Self::List => "list",
            Self::ListItem => "list_item",
            Self::Tree => "tree",
            Self::TreeItem => "tree_item",
            Self::Tab => "tab",
            Self::TabItem => "tab_item",
            Self::Menu => "menu",
            Self::MenuItem => "menu_item",
            Self::Link => "link",
            Self::Image => "image",
            Self::Table => "table",
            Self::Row => "row",
            Self::Cell => "cell",
            Self::Document => "document",
            Self::Text => "text",
        })
    }
}

/// 一个已完成类型检查的属性比较条件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyPredicate {
    /// 要读取的 portable 或显式后端属性。
    pub attribute: SelectorAttribute,
    /// 要执行的比较操作。
    pub operator: MatchOperator,
    /// 与属性类型及运算符一致的右值。
    pub value: PredicateValue,
}

/// AQL 可查询的 portable 与后端专用属性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "type", content = "attribute", rename_all = "snake_case")]
pub enum SelectorAttribute {
    /// Accessible Name。
    Name,
    /// 跨平台逻辑元素键。
    Key,
    /// 元素当前值。
    Value,
    /// 元素是否可交互。
    Enabled,
    /// 元素是否可见。
    Visible,
    /// 元素是否拥有焦点。
    Focused,
    /// 元素是否已勾选。
    Checked,
    /// 元素是否已选择。
    Selected,
    /// Windows UIA 专用属性。
    Uia(UiaAttribute),
    /// 浏览器 DOM 专用属性。
    Dom(DomAttribute),
}

impl SelectorAttribute {
    /// 判断属性是否要求布尔右值。
    pub const fn is_boolean(self) -> bool {
        matches!(
            self,
            Self::Enabled | Self::Visible | Self::Focused | Self::Checked | Self::Selected
        )
    }
}

impl fmt::Display for SelectorAttribute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Name => "name",
            Self::Key => "key",
            Self::Value => "value",
            Self::Enabled => "enabled",
            Self::Visible => "visible",
            Self::Focused => "focused",
            Self::Checked => "checked",
            Self::Selected => "selected",
            Self::Uia(UiaAttribute::AutomationId) => "uia.automation_id",
            Self::Uia(UiaAttribute::ClassName) => "uia.class_name",
            Self::Uia(UiaAttribute::AcceleratorKey) => "uia.accelerator_key",
            Self::Uia(UiaAttribute::AccessKey) => "uia.access_key",
            Self::Uia(UiaAttribute::FrameworkId) => "uia.framework_id",
            Self::Dom(DomAttribute::TestId) => "dom.test_id",
            Self::Dom(DomAttribute::Class) => "dom.class",
        })
    }
}

/// AQL v1 显式开放的 Windows UIA 属性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiaAttribute {
    /// UI Automation AutomationId。
    AutomationId,
    /// UI Automation ClassName。
    ClassName,
    /// UI Automation AcceleratorKey，例如命令快捷键 `Ctrl+F`。
    AcceleratorKey,
    /// UI Automation AccessKey，例如菜单助记键。
    AccessKey,
    /// UI Automation provider framework 标识。
    FrameworkId,
}

/// AQL v1 显式开放的 DOM 属性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomAttribute {
    /// `data-testid` 测试标识。
    TestId,
    /// DOM class token 字符串。
    Class,
}

/// AQL v1 的比较运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchOperator {
    /// 完全相等。
    Equal,
    /// 不相等。
    NotEqual,
    /// 包含子串。
    Contains,
    /// 以前缀开头。
    StartsWith,
    /// 以后缀结尾。
    EndsWith,
    /// 以正则表达式匹配。
    Regex,
}

impl fmt::Display for MatchOperator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Equal => "=",
            Self::NotEqual => "!=",
            Self::Contains => "contains",
            Self::StartsWith => "starts_with",
            Self::EndsWith => "ends_with",
            Self::Regex => "matches",
        })
    }
}

/// 属性谓词经类型检查后的右值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PredicateValue {
    /// 普通 Unicode 文本。
    Text(String),
    /// 布尔状态。
    Boolean(bool),
    /// 正则表达式字面量。
    Regex(RegexLiteral),
    /// 由 Runtime 在 prepare 阶段解析的文本参数。
    Parameter(QueryParameter),
}

/// AQL 源码中的强类型参数引用。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct QueryParameter {
    /// 不含 `$` 前缀的参数名。
    pub name: String,
    /// 由属性上下文确定的期望值类型。
    pub expected_type: QueryValueType,
}

/// 当前 AQL 参数绑定支持的值类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryValueType {
    /// Unicode 文本。
    Text,
}

/// 不依赖具体正则实现的 AQL 正则字面量。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegexLiteral {
    /// 不含 `/` 分隔符的正则模式。
    pub pattern: String,
    /// 是否启用 Unicode 大小写不敏感匹配，对应 `i` 标志。
    pub case_insensitive: bool,
}
