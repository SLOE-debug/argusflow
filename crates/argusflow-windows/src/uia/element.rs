//! UIA 查询模型和不携带 COM 的元素租约。
use crate::WindowIdentity;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// UIA 标准控件类型，数值与 Windows UI Automation 标准一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ControlType {
    /// 按钮。
    Button = 50000,
    /// 复选框。
    CheckBox = 50002,
    /// 下拉框。
    ComboBox = 50003,
    /// 编辑框。
    Edit = 50004,
    /// 超链接。
    Hyperlink = 50005,
    /// 列表项。
    ListItem = 50007,
    /// 列表。
    List = 50008,
    /// 菜单。
    Menu = 50009,
    /// 菜单项。
    MenuItem = 50011,
    /// 单选框。
    RadioButton = 50013,
    /// 滚动条。
    ScrollBar = 50014,
    /// 标签页容器。
    Tab = 50018,
    /// 标签页。
    TabItem = 50019,
    /// 文本。
    Text = 50020,
    /// 树。
    Tree = 50023,
    /// 树节点。
    TreeItem = 50024,
    /// 自定义控件。
    Custom = 50025,
    /// 分组。
    Group = 50026,
    /// 文档。
    Document = 50030,
    /// 窗口。
    Window = 50032,
    /// 面板。
    Pane = 50033,
}

/// 类型化精确条件；布尔组合不依赖查询字符串。
#[derive(Debug, Clone)]
pub enum Predicate {
    /// 所有元素。
    Any,
    /// AutomationId 精确相等。
    AutomationId(String),
    /// Accessible Name 精确相等。
    Name(String),
    /// ClassName 精确相等。
    ClassName(String),
    /// 标准控件类型。
    ControlType(ControlType),
    /// 所有条件成立。
    All(Vec<Predicate>),
    /// 任一条件成立。
    OneOf(Vec<Predicate>),
    /// 条件取反。
    Not(Box<Predicate>),
}

/// 只在已验证窗口内搜索。
#[derive(Debug, Clone, Copy)]
pub enum SearchScope {
    /// 窗口根节点。
    Root,
    /// 窗口的直接子元素。
    Children,
    /// 窗口内全部后代。
    Descendants,
}

/// 元素查询及资源预算。
#[derive(Debug, Clone)]
pub struct Query {
    /// 窗口范围。
    pub window: WindowIdentity,
    /// 要求满足的条件。
    pub predicate: Predicate,
    /// 搜索深度语义。
    pub scope: SearchScope,
}

/// 普通 Rust 属性快照；未知控件类型保留原生数字，不伪造类型。
#[derive(Debug, Clone)]
pub struct ElementSnapshot {
    /// UIA Name。
    pub name: String,
    /// UIA AutomationId。
    pub automation_id: String,
    /// UIA ClassName。
    pub class_name: String,
    /// 原生 UIA ControlType 标识。
    pub control_type: i32,
    /// 是否启用。
    pub enabled: bool,
    /// 是否在屏幕之外。
    pub offscreen: bool,
    /// 虚拟桌面物理像素边界：[left, top, right, bottom]。
    pub bounds: [i32; 4],
}

#[derive(Debug)]
pub(crate) struct Lease {
    pub(crate) alive: AtomicBool,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
    }
}

/// 绑定 runtime、窗口和有限时长的元素句柄；最后一个副本释放后失效。
#[derive(Debug, Clone)]
pub struct ElementHandle {
    pub(crate) runtime: u64,
    pub(crate) id: u64,
    pub(crate) window: WindowIdentity,
    pub(crate) lease: Arc<Lease>,
}
impl ElementHandle {
    /// 元素所属的窗口身份。
    pub fn window(&self) -> WindowIdentity {
        self.window.clone()
    }
    /// 提前撤销全部句柄副本的租约。
    pub fn release(&self) {
        self.lease.alive.store(false, Ordering::Release);
    }
}
