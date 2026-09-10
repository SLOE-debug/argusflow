//! 后端之间共享的封闭角色与属性词汇表。
use super::ValueType;
use serde::{Deserialize, Serialize};

macro_rules! symbols {
    ($name:ident { $($(#[$meta:meta])* $variant:ident => $word:literal),+ $(,)? }) => {
        /// AQL 中使用的稳定领域符号；英文名称是源码契约。
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub enum $name { $($(#[$meta])* $variant),+ }
        impl $name {
            /// 全部支持的符号，按声明顺序返回。
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            /// 规范英文拼写。
            pub const fn name(self) -> &'static str { match self { $(Self::$variant => $word),+ } }
            /// 仅识别完整英文拼写。
            pub fn parse(word: &str) -> Option<Self> { match word { $($word => Some(Self::$variant)),+, _ => None } }
        }
    };
}
symbols!(Role {
    /// 任意角色的真实元素。
    Element => "element",
    /// 窗口。
    Window => "window",
    /// 对话框。
    Dialog => "dialog",
    /// 面板。
    Pane => "pane",
    /// 按钮。
    Button => "button",
    /// 可编辑文本。
    TextBox => "textbox",
    /// 复选框。
    CheckBox => "checkbox",
    /// 单选框。
    Radio => "radio",
    /// 组合框。
    ComboBox => "combobox",
    /// 列表。
    List => "list",
    /// 列表项。
    ListItem => "list_item",
    /// 树。
    Tree => "tree",
    /// 树项。
    TreeItem => "tree_item",
    /// 选项卡容器。
    Tab => "tab",
    /// 选项卡项。
    TabItem => "tab_item",
    /// 菜单。
    Menu => "menu",
    /// 菜单项。
    MenuItem => "menu_item",
    /// 链接。
    Link => "link",
    /// 图片。
    Image => "image",
    /// 表格。
    Table => "table",
    /// 表格行。
    Row => "row",
    /// 单元格。
    Cell => "cell",
    /// 文档。
    Document => "document",
    /// 静态文字。
    Text => "text"
});
symbols!(Attribute {
    /// 可访问名称；OCR 为识别文字。
    Name => "name",
    /// 文字内容。
    Text => "text",
    /// 来源的逻辑标识。
    Key => "key",
    /// 控件当前值。
    Value => "value",
    /// 是否启用。
    Enabled => "enabled",
    /// 后端报告的可见状态，不承诺未被遮挡。
    Visible => "visible",
    /// 是否聚焦。
    Focused => "focused",
    /// 勾选状态，混合状态缺失。
    Checked => "checked",
    /// 选中状态。
    Selected => "selected",
    /// OCR 置信度，范围 0..=1。
    Confidence => "confidence",
    /// UIA AutomationId。
    AutomationId => "uia.automation_id",
    /// UIA ClassName。
    ClassName => "uia.class_name",
    /// UIA AcceleratorKey。
    AcceleratorKey => "uia.accelerator_key",
    /// UIA AccessKey。
    AccessKey => "uia.access_key",
    /// UIA FrameworkId。
    FrameworkId => "uia.framework_id",
    /// DOM id。
    DomId => "dom.id",
    /// DOM 测试标识。
    TestId => "dom.test_id",
    /// DOM class 属性。
    DomClass => "dom.class",
    /// 小写 DOM 标签名。
    Tag => "dom.tag"
});
impl Attribute {
    /// 属性比较和参数绑定需要的值类型。
    pub const fn value_type(self) -> ValueType {
        match self {
            Self::Enabled | Self::Visible | Self::Focused | Self::Checked | Self::Selected => {
                ValueType::Boolean
            }
            Self::Confidence => ValueType::Number,
            _ => ValueType::Text,
        }
    }
}
