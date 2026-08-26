//! 已由 AQL compiler 证明可由 Windows UI Automation 执行的原生查询 IR。

use regex::{Regex, RegexBuilder};

/// ArgusFlow 支持物化的 UIA ControlType。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiaControlType {
    /// 顶层窗口。
    Window,
    /// 通用面板。
    Pane,
    /// 可调用按钮。
    Button,
    /// 可编辑文本框。
    Edit,
    /// 复选框。
    CheckBox,
    /// 单选按钮。
    RadioButton,
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
    /// 超链接。
    Hyperlink,
    /// 图像。
    Image,
    /// 表格容器。
    Table,
    /// 文档区域。
    Document,
    /// 静态文本。
    Text,
}

/// 一个已完整降级到 UIA 语义的角色约束。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiaRoleConstraint {
    /// 通过单个 UIA ControlType 匹配。
    ControlType(UiaControlType),
    /// 通过 Window ControlType 与 IsDialog 联合匹配。
    Dialog,
}

/// UIA executor 可以物化为 property id 的属性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum UiaProperty {
    /// Accessible Name。
    Name,
    /// AutomationId。
    AutomationId,
    /// 原生窗口类名。
    ClassName,
    /// 命令快捷键，例如 `Ctrl+F`。
    AcceleratorKey,
    /// 菜单或控件助记键。
    AccessKey,
    /// provider framework 标识。
    FrameworkId,
    /// ValuePattern 的 Value 属性。
    Value,
    /// 元素是否可交互。
    IsEnabled,
    /// 元素是否位于屏幕外。
    IsOffscreen,
    /// 元素是否拥有键盘焦点。
    HasKeyboardFocus,
    /// TogglePattern 的 ToggleState。
    ToggleState,
    /// SelectionItemPattern 的 IsSelected。
    IsSelected,
    /// 元素是否为对话框。
    IsDialog,
}

/// UIA 原生 PropertyCondition 可接受的强类型右值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiaNativeValue {
    /// BSTR 文本值。
    Text(String),
    /// VARIANT_BOOL 布尔值。
    Boolean(bool),
    /// 32 位整数或枚举值。
    Integer(i32),
}

/// 可直接物化为 UIA 原生 condition 的比较。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiaNativeComparison {
    /// PropertyCondition 等值判断。
    Equal(UiaNativeValue),
    /// Not(PropertyCondition) 不等判断。
    NotEqual(UiaNativeValue),
}

/// UIA 原生属性谓词；不存在 AQL 属性或值的运行时再映射。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiaNativePredicate {
    /// 已映射的 UIA 属性。
    pub property: UiaProperty,
    /// 已映射的原生比较。
    pub comparison: UiaNativeComparison,
}

/// CacheRequest 需要读取的单个属性投影。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct UiaPropertyProjection {
    /// 对应的 UIA 属性。
    property: UiaProperty,
}

impl UiaPropertyProjection {
    /// 从已经证明可读取的 UIA 属性创建缓存投影。
    pub const fn new(property: UiaProperty) -> Self {
        Self { property }
    }

    /// 返回该投影对应的只读 UIA 属性。
    pub const fn property(self) -> UiaProperty {
        self.property
    }
}

/// UIA 无法原生表达、但可对缓存值本地计算的匹配器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiaResidualMatcher {
    /// 包含指定文本。
    Contains(String),
    /// 以指定文本开头。
    StartsWith(String),
    /// 以指定文本结尾。
    EndsWith(String),
    /// 使用 UIA query compiler 在 request 级预编译的正则模式。
    Regex(UiaResidualRegex),
}

/// 保留可解释源码并持有一次编译结果的 UIA residual 正则。
#[derive(Debug, Clone)]
pub struct UiaResidualRegex {
    /// 不含字面量分隔符的正则源码。
    pattern: String,
    /// 是否忽略 Unicode 大小写。
    case_insensitive: bool,
    /// query compiler 创建、供所有候选共享的正则执行器。
    compiled: Regex,
}

impl UiaResidualRegex {
    /// 编译一条由 AQL parser 校验过的 residual 正则。
    pub fn new(pattern: &str, case_insensitive: bool) -> Result<Self, regex::Error> {
        let compiled = RegexBuilder::new(pattern)
            .case_insensitive(case_insensitive)
            .build()?;
        Ok(Self {
            pattern: pattern.to_owned(),
            case_insensitive,
            compiled,
        })
    }

    /// 对缓存属性执行已经编译的正则匹配。
    pub(crate) fn is_match(&self, value: &str) -> bool {
        self.compiled.is_match(value)
    }
}

impl PartialEq for UiaResidualRegex {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern && self.case_insensitive == other.case_insensitive
    }
}

impl Eq for UiaResidualRegex {}

/// 使用 CacheRequest 投影在 Rust 中求值的 residual 谓词。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiaResidualPredicate {
    /// 一次性缓存的属性投影。
    pub projection: UiaPropertyProjection,
    /// 对缓存值执行的本地比较。
    pub matcher: UiaResidualMatcher,
}
