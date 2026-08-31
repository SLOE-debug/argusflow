//! 跨后端 UI 事实、观察请求与三态结果契约。
//!
//! 本模块只描述平台无关的数据边界。选择器执行、页面或窗口访问以及重试编排由
//! Runtime 和具体观察后端负责。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AqlQuery, BackendKind, BackendPolicy, TargetScope, UiQuery};

/// UI 实体边界框使用的明确坐标空间。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateSpace {
    /// Windows 虚拟屏幕中的物理像素。
    ScreenPhysical,
    /// 浏览器布局视口中的 CSS 像素。
    BrowserViewportCss,
}

/// 带坐标空间的实体边界，避免桌面物理像素与浏览器 CSS 像素混用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityBounds {
    /// 边界所属的坐标空间。
    pub space: CoordinateSpace,
    /// 左上角横坐标。
    pub x: f64,
    /// 左上角纵坐标。
    pub y: f64,
    /// 非负宽度。
    pub width: f64,
    /// 非负高度。
    pub height: f64,
}

/// 统一实体来自哪一种已注册观察事实源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntitySource {
    /// Windows UI Automation 元素。
    WindowsUia,
    /// 浏览器 DOM 或 Accessibility 元素。
    BrowserCdp,
    /// OCR 文字区域。
    Ocr,
}

/// UIA、DOM/AX 与 OCR 共用的最小只读实体快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySnapshot {
    /// Accessible Name 或等价可见名称。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 面向用户显示的文本。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 控件或表单值。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// 跨后端语义角色名称。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// 实体在明确坐标空间中的边界。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<EntityBounds>,
    /// 后端提供时的置信度，范围为 `0.0..=1.0`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// 产生当前事实的后端来源。
    pub source: EntitySource,
}

/// `project` 允许读取的固定字段集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityField {
    /// Accessible Name。
    Name,
    /// 可见文字。
    Text,
    /// 控件值。
    Value,
    /// 语义角色。
    Role,
    /// 带坐标空间的边界。
    Bounds,
    /// 观察置信度。
    Confidence,
    /// 事实来源。
    Source,
}

/// AQL v3 顶层表达式的静态结果类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationValueType {
    /// 统一 UI 实体集合。
    Entities,
    /// 固定字段投影产生的记录集合。
    Records,
    /// 非负实体数量。
    Number,
    /// 可用于三端口分支的布尔事实。
    Boolean,
}

/// 数量表达式支持的比较运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberComparison {
    /// 等于。
    Equal,
    /// 不等于。
    NotEqual,
    /// 大于。
    GreaterThan,
    /// 大于或等于。
    GreaterThanOrEqual,
    /// 小于。
    LessThan,
    /// 小于或等于。
    LessThanOrEqual,
}

/// 数量比较右值；参数在 Runtime prepare 阶段解析。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum NumberOperand {
    /// 非负整数字面量。
    Literal(u64),
    /// 不含 `$` 前缀的动态整数参数名。
    Parameter(String),
}

/// AQL v3 的观察表达式；选择器叶节点仍使用稳定 `UiQuery`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObservationExpr {
    /// 返回选择器命中的实体集合。
    Entities {
        /// 由单一后端执行的选择器。
        query: UiQuery,
    },
    /// 将实体集合限制到固定字段记录。
    Project {
        /// 由单一后端执行的选择器。
        query: UiQuery,
        /// 非空且不重复的字段集合。
        fields: Vec<EntityField>,
    },
    /// 返回精确实体数量。
    Count {
        /// 由单一后端执行的选择器。
        query: UiQuery,
    },
    /// 判断至少存在一个实体。
    Exists {
        /// 由单一后端执行的选择器。
        query: UiQuery,
    },
    /// 比较数量表达式与非负整数。
    Compare {
        /// 必须产生 Number 的左表达式。
        left: Box<ObservationExpr>,
        /// 确定性数值比较运算符。
        operator: NumberComparison,
        /// 已冻结字面量或待解析参数。
        right: NumberOperand,
    },
    /// 使用强三值逻辑计算全部条件。
    AllOf {
        /// 至少两个布尔表达式。
        expressions: Vec<ObservationExpr>,
    },
    /// 使用强三值逻辑计算任一条件。
    AnyOf {
        /// 至少两个布尔表达式。
        expressions: Vec<ObservationExpr>,
    },
    /// 对一个布尔表达式取反。
    Not {
        /// 被取反的布尔表达式。
        expression: Box<ObservationExpr>,
    },
}

impl ObservationExpr {
    /// 返回表达式通过语法构造即可确定的结果类型。
    pub const fn value_type(&self) -> ObservationValueType {
        match self {
            Self::Entities { .. } => ObservationValueType::Entities,
            Self::Project { .. } => ObservationValueType::Records,
            Self::Count { .. } => ObservationValueType::Number,
            Self::Exists { .. }
            | Self::Compare { .. }
            | Self::AllOf { .. }
            | Self::AnyOf { .. }
            | Self::Not { .. } => ObservationValueType::Boolean,
        }
    }
}

/// 已解析并完成类型检查的 AQL v3 观察查询。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationQuery {
    /// 顶层强类型观察表达式。
    pub expression: ObservationExpr,
}

impl ObservationQuery {
    /// 从已验证表达式创建观察查询。
    pub const fn new(expression: ObservationExpr) -> Self {
        Self { expression }
    }

    /// 返回顶层表达式的静态结果类型。
    pub const fn value_type(&self) -> ObservationValueType {
        self.expression.value_type()
    }
}

/// 一次观察中后端返回的选择器事实与覆盖完整性。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityObservation {
    /// 当前后端在同一状态快照中解析到的实体。
    pub entities: Vec<EntitySnapshot>,
    /// 是否完整覆盖查询作用域；只有完整覆盖才能证明精确数量或不存在。
    pub complete: bool,
}

/// Known 观察结果携带的强类型值。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ObservationValue {
    /// 统一实体集合。
    Entities(Vec<EntitySnapshot>),
    /// 固定字段记录集合；字段名来自 `EntityField` 的 snake_case 名称。
    Records(Vec<BTreeMap<String, Value>>),
    /// 精确非负数量。
    Number(u64),
    /// 确定布尔事实。
    Boolean(bool),
}

impl ObservationValue {
    /// 返回值的稳定静态类型。
    pub const fn value_type(&self) -> ObservationValueType {
        match self {
            Self::Entities(_) => ObservationValueType::Entities,
            Self::Records(_) => ObservationValueType::Records,
            Self::Number(_) => ObservationValueType::Number,
            Self::Boolean(_) => ObservationValueType::Boolean,
        }
    }
}

/// 观察无法得出确定事实时使用的稳定原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationUnknownReason {
    /// 后端运行环境、会话或窗口不可用。
    BackendUnavailable,
    /// 只观察了作用域的一部分，无法证明精确数量或否定事实。
    IncompleteCoverage,
    /// 观察预算在获得完整事实前耗尽。
    Timeout,
    /// 后端返回的事实不符合冻结契约。
    InvalidResponse,
}

/// Observe 节点唯一公开的 Known/Unknown 判别联合。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ObservationResult {
    /// 已获得可作为业务事实使用的确定结果。
    Known {
        /// 实际产生事实的单一后端。
        backend: BackendKind,
        /// 与 AQL 顶层类型一致的观察值。
        value: ObservationValue,
    },
    /// 无法证明真、假、数量或完整集合；不得退化为 false、0 或空数组。
    Unknown {
        /// 最后尝试的后端；在没有候选时为空。
        backend: Option<BackendKind>,
        /// 稳定原因码。
        reason: ObservationUnknownReason,
        /// 有界观察是否可以安全重试。
        retryable: bool,
    },
}

/// Observe 节点持久化的单次或有界观察策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ObservationPolicy {
    /// 只读取一次事实快照。
    Once,
    /// 只重试可重试 Unknown，Known(false) 不会触发重试。
    Bounded {
        /// 整个观察生命周期的毫秒预算。
        timeout_ms: u64,
        /// 两次完整观察之间的毫秒间隔。
        poll_interval_ms: u64,
    },
}

/// Runtime 交给观察路由器的强类型请求。
#[derive(Debug, Clone, PartialEq)]
pub struct ObservationRequest {
    /// 已解析的资源作用域。
    pub scope: TargetScope,
    /// 参数已经冻结的 AQL v3 查询。
    pub query: ObservationQuery,
    /// 原始源码只用于安全诊断与 Trace 关联。
    pub source: String,
    /// 观察后端集合约束与偏好。
    pub backend_policy: BackendPolicy,
}

/// Observe 节点的持久化业务定义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObserveSpec {
    /// 当前上下文或显式应用/浏览器资源。
    pub scope: TargetScope,
    /// AQL v3 源码和动态参数绑定。
    pub query: AqlQuery,
    /// 单一事实源的后端候选策略。
    pub backend_policy: BackendPolicy,
    /// Unknown 的有限重试策略。
    pub policy: ObservationPolicy,
}
