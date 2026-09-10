//! 与当前领域类型逐项对应的编辑说明，不依赖平台或中文关键字转换。
use crate::{Attribute, Role, ValueType};

pub(super) fn value_type(value: ValueType) -> &'static str {
    match value {
        ValueType::Text => "文本",
        ValueType::Boolean => "布尔",
        ValueType::Number => "数值",
    }
}
pub(super) fn role(role: Role) -> &'static str {
    match role {
        Role::Element => {
            "查找任意角色的元素。括号可为空，也可填写属性条件；普通层级查询不跨越文档边界。"
        }
        Role::Window => "查找窗口，只支持 UIA 来源。常用于限定后续控件查询的范围。",
        Role::Dialog => "查找对话框，支持 UIA 和浏览器来源。",
        Role::Pane => "查找面板或分组容器，支持 UIA 和浏览器来源。",
        Role::Button => "查找按钮，支持 UIA 和浏览器来源。可按名称、可用状态等条件筛选。",
        Role::TextBox => {
            "查找文本输入框，支持 UIA 和浏览器来源。是否可实际输入会在执行动作时重新检查。"
        }
        Role::CheckBox => "查找复选框，可使用 checked 属性筛选勾选状态。",
        Role::Radio => "查找单选框，支持 UIA 和浏览器来源。",
        Role::ComboBox => "查找组合框或下拉选择控件，支持 UIA 和浏览器来源。",
        Role::List => "查找列表容器，可继续通过 > 或 >> 查询列表项。",
        Role::ListItem => "查找列表项，支持 UIA 和浏览器来源。",
        Role::Tree => "查找树形控件容器，可继续查询树项。",
        Role::TreeItem => "查找树形控件中的项目，支持 UIA 和浏览器来源。",
        Role::Tab => "查找选项卡组，可继续查询其中的选项卡。",
        Role::TabItem => "查找单个选项卡，可使用 selected 属性筛选选中状态。",
        Role::Menu => "查找菜单或菜单栏，支持 UIA 和浏览器来源。",
        Role::MenuItem => "查找菜单项，支持 UIA 和浏览器来源。",
        Role::Link => "查找链接，支持 UIA 和浏览器来源。",
        Role::Image => "查找图片元素，支持 UIA 和浏览器来源。",
        Role::Table => "查找表格或网格容器，可继续查询行和单元格。",
        Role::Row => "查找表格行，支持 UIA 和浏览器来源。",
        Role::Cell => "查找表格单元格，支持 UIA 和浏览器来源。",
        Role::Document => "查找文档元素，支持 UIA 和浏览器来源。iframe 文档需通过 frame 显式进入。",
        Role::Text => {
            "查找静态文字或 OCR 文字块，支持 UIA、浏览器和 OCR 来源；OCR 不推断控件角色。"
        }
    }
}
pub(super) fn attribute(attribute: Attribute) -> &'static str {
    use Attribute::*;
    match attribute {
        Name => {
            "元素的可访问名称。UIA/浏览器使用来源提供的名称，OCR 使用识别文字。支持精确比较、包含、前后缀和正则。"
        }
        Text => {
            "元素的文字内容。UIA 读取静态文字或 Text Pattern，浏览器读取 DOM 文字，OCR 使用识别文字。"
        }
        Key => "来源内的逻辑标识：UIA 对应 AutomationId，浏览器对应 id。OCR 不支持此属性。",
        Value => "控件的当前值，支持 UIA 和浏览器。没有值的元素保持属性缺失，不当作空字符串。",
        Enabled => "控件是否可用。支持 UIA 和浏览器；true 表示可用，false 表示禁用。",
        Visible => {
            "来源报告的可见状态，支持 UIA 和浏览器。可见不代表未被遮挡，点击时仍会检查位置。"
        }
        Focused => "元素当前是否具有键盘焦点，支持 UIA 和浏览器。",
        Checked => {
            "复选或切换控件的勾选状态，支持 UIA 和浏览器。混合状态或不提供该属性时保持缺失。"
        }
        Selected => "项目是否选中，支持 UIA 和浏览器。不提供选中状态的元素保持属性缺失。",
        Confidence => {
            "OCR 文字块的识别置信度，范围为 0 到 1。仅支持 OCR，可用 >= 等数值运算符筛选。"
        }
        AutomationId => "UIA AutomationId，通常由应用提供。仅支持 UIA；可用于区分名称相同的控件。",
        ClassName => "UIA 控件类名 ClassName，例如 Edit 或 Button。仅支持 UIA。",
        AcceleratorKey => "UIA 报告的控件快捷键文字，例如 Ctrl+S。仅支持 UIA。",
        AccessKey => "UIA 报告的访问键文字，例如 Alt+F。仅支持 UIA。",
        FrameworkId => "UIA 报告的界面框架标识，例如 Win32 或 WPF。仅支持 UIA。",
        DomId => "DOM 元素的 id 属性。仅支持浏览器；未设置 id 时保持属性缺失。",
        TestId => "DOM 元素的 data-testid 属性，常用于稳定测试定位。仅支持浏览器。",
        DomClass => {
            "DOM 元素完整的 class 属性文字。仅支持浏览器；匹配某个独立 class 可使用 css(\".类名\")。"
        }
        Tag => "DOM 元素的小写标签名，例如 button、input、iframe。仅支持浏览器。",
    }
}
pub(super) fn attribute_example(attribute: Attribute) -> String {
    let value = match attribute {
        Attribute::ClassName => "\"Edit\"",
        Attribute::FrameworkId => "\"Win32\"",
        Attribute::AcceleratorKey => "\"Ctrl+S\"",
        Attribute::AccessKey => "\"Alt+F\"",
        Attribute::DomClass => "\"save-button primary\"",
        Attribute::Tag => "\"button\"",
        Attribute::Key | Attribute::AutomationId | Attribute::DomId | Attribute::TestId => {
            "\"save\""
        }
        _ => match attribute.value_type() {
            ValueType::Text => "\"保存\"",
            ValueType::Boolean => "true",
            ValueType::Number => "0.8",
        },
    };
    format!("{} = {value}", attribute.name())
}
