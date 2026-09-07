//! 物理输入反查语义的只读契约；不依赖录制器、平台或执行路由。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ElementRole, ResourceId, ScreenPoint, WindowIdentity};

/// 带明确坐标空间的检查矩形；物理屏幕允许负坐标。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InspectionRect {
    /// 左边界。
    pub x: f64,
    /// 上边界。
    pub y: f64,
    /// 宽度。
    pub width: f64,
    /// 高度。
    pub height: f64,
}

impl InspectionRect {
    /// 检查非空有限矩形是否包含物理屏幕点。
    pub fn contains(self, point: ScreenPoint) -> bool {
        self.is_valid()
            && f64::from(point.x) >= self.x
            && f64::from(point.y) >= self.y
            && f64::from(point.x) < self.x + self.width
            && f64::from(point.y) < self.y + self.height
    }

    /// 拒绝 NaN、无限值和退化几何。
    pub fn is_valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f64::is_finite)
            && self.width > 0.0
            && self.height > 0.0
    }
}

/// Worker 从实际目标 HWND 读取的瞬时上下文，不以焦点窗口替代点击窗口。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectionContext {
    /// 根窗口身份，PID 用于拒绝句柄复用。
    pub window: WindowIdentity,
    /// 当前进程 EXE 完整路径；权限不足时为空。
    pub executable_path: Option<String>,
    /// 窗口标题，不包含控件值。
    pub title: String,
    /// 根窗口类名。
    pub class_name: String,
    /// 根窗口屏幕物理矩形。
    pub bounds: InspectionRect,
    /// 原生 Chromium renderer client 的屏幕物理矩形；缺失时禁止猜测工具栏高度。
    pub browser_viewport: Option<InspectionRect>,
    /// 窗口 DPI，只用于长度转换，绝不能用于缩放虚拟屏幕原点。
    pub dpi: u32,
    /// 仅用于 CDP 活动页面交叉验证；绝不用于替代 Point 的窗口定位。
    pub has_keyboard_focus: bool,
}

/// 点击坐标与当前键盘焦点是两种不同的反查请求。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "point", rename_all = "snake_case")]
pub enum InspectionProbe {
    /// mouse down 冻结的物理坐标。
    Point(ScreenPoint),
    /// 键盘输入前当前 focused element。
    Focus,
    /// 事件指定的窗口根；使用 InspectionContext 的身份，不借用稍后的焦点。
    Window,
}

/// 可序列化的语义属性；刻意不提供 value、outerHTML 或任意属性容器。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ElementSemantics {
    /// 可由 AQL 表达的角色；未知角色保留为 None。
    pub role: Option<ElementRole>,
    /// Accessible name，敏感元素会清空。
    pub name: Option<String>,
    /// UIA AutomationId。
    pub automation_id: Option<String>,
    /// DOM data-testid。
    pub test_id: Option<String>,
    /// DOM id。
    pub stable_id: Option<String>,
    /// UIA ClassName 或 DOM class 原文。
    pub class_name: Option<String>,
    /// UIA FrameworkId。
    pub framework_id: Option<String>,
}

/// 字段敏感性三态；未知时录制器必须遮盖键盘内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldSensitivity {
    /// 后端成功检查了受保护属性与字段元数据。
    Normal,
    /// Password 或敏感字段。
    Sensitive,
    /// 无法可靠判断。
    Unknown,
}

/// 不含平台对象的语义检查结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectedEntity {
    /// UIA runtime id、DOM backendNodeId 或 Scene node id，只用于录制期身份比较。
    pub identity: String,
    /// 当前目标的语义属性。
    pub semantics: ElementSemantics,
    /// 由近到远的有限祖先链。
    pub ancestors: Vec<ElementSemantics>,
    /// 屏幕物理矩形。
    pub bounds: InspectionRect,
    /// 是否可编辑，不代表能够执行 SetValue。
    pub editable: bool,
    /// 敏感性结果。
    pub sensitivity: FieldSensitivity,
    /// managed CDP 资源身份，其他后端为 None。
    pub browser_session: Option<ResourceId>,
    /// 已移除 query/hash/userinfo 的页面地址。
    pub page_url: Option<String>,
    /// 后端置信度，范围 0..=1。
    pub confidence: f32,
}

/// 反查失败的封闭分类；不携带可能含页面值的原始 provider 错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionFailure {
    /// 没有当前已附加且属于目标进程的页面。
    #[error("no managed attached session for this window")]
    UnmanagedWindow,
    /// HWND、PID、target 或焦点身份变化。
    #[error("window or target identity changed")]
    ContextChanged,
    /// 无法验证坐标变换或点不在网页 viewport 中。
    #[error("viewport geometry could not be verified")]
    InvalidGeometry,
    /// 目标不在当前后端可观察的树中。
    #[error("no valid semantic target at this location")]
    NoElement,
    /// 平台 provider 或连接不可用。
    #[error("inspection backend unavailable")]
    Unavailable,
    /// 有限预算到期。
    #[error("inspection deadline exceeded")]
    Timeout,
    /// Iframe 或 shadow scope 尚不能由现有执行器精确重放。
    #[error("target is outside the replayable document scope")]
    UnsupportedScope,
}

/// 平台窗口反查边界；只允许 worker 调用。
pub trait WindowInspector: Send + Sync {
    /// Point 使用 WindowFromPoint + GA_ROOT，Focus 才允许使用焦点窗口。
    fn context(&self, probe: InspectionProbe) -> Result<InspectionContext, InspectionFailure>;
    /// 对窗口生命周期事件使用事件自带的 HWND/PID，不能替换为当前前台窗口。
    fn window_context(
        &self,
        window: WindowIdentity,
    ) -> Result<InspectionContext, InspectionFailure>;
}

/// CDP/UIA/Vision 共享的只读反查边界，不注册到 ActionRouter。
#[async_trait]
pub trait TargetInspector: Send + Sync {
    /// 返回语义快照；实现必须验证请求仍属于该窗口与后端。
    async fn inspect(
        &self,
        context: &InspectionContext,
        probe: InspectionProbe,
    ) -> Result<InspectedEntity, InspectionFailure>;
}

/// 在读取控件值前，根据声明字段元数据执行保守的敏感分类。
pub fn sensitive_field_metadata(value: &str) -> bool {
    let normalized = value.to_lowercase().replace(['-', '_', ' '], "");
    [
        "password",
        "passwd",
        "passcode",
        "secret",
        "token",
        "apikey",
        "creditcard",
        "cardnumber",
        "ccnumber",
        "cccsc",
        "cvv",
        "cvc",
        "otp",
        "onetimecode",
        "ssn",
        "密码",
        "口令",
        "密钥",
        "验证码",
        "身份证",
        "银行卡",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}
