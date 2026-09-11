//! 有限控制结构与开放业务任务。
use crate::{Expr, Value, ValueType};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 可由 Catch 或重试策略匹配的执行错误类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// 用户定义的失败。
    User,
    /// 运行表达式失败，例如越界或除零。
    Expression,
    /// 单个节点或结构超时。
    Timeout,
    /// 暂时没有容量。
    Busy,
    /// 外部能力不可用。
    Unavailable,
    /// 未找到目标。
    NotFound,
    /// 多个目标。
    Ambiguous,
    /// 资源已经失效。
    Stale,
    /// 外部操作错误。
    Operation,
    /// 资源释放失败。
    Cleanup,
    /// 用户取消；业务不可捕获。
    Cancelled,
    /// 运行总时限；业务不可捕获。
    RunTimeout,
    /// 引擎预算耗尽；业务不可捕获。
    Limit,
    /// 扩展实现违反契约；业务不可捕获。
    Contract,
}
impl ErrorKind {
    /// 是否允许由流程 Catch 显式处理。
    pub fn catchable(self) -> bool {
        !matches!(
            self,
            Self::Cancelled | Self::RunTimeout | Self::Limit | Self::Contract
        )
    }
}

/// 一次原子赋值中的一个目标。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    /// 沿词法作用域查找的最近声明。
    pub name: String,
    /// 原子事务开始时求值的右值。
    pub value: Expr,
}

/// 节点的控制流位置及可选的总时限。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// 文档内唯一节点 ID。
    pub id: String,
    /// 本作用域内下一节点；None 表示正常离开。
    pub next: Option<String>,
    /// 可选节点总时限，含全部重试和子作用域，单位毫秒。
    pub timeout_ms: Option<u64>,
    /// 控制或业务行为。
    pub action: Action,
    /// 从原生结果计算的附加输出；不能覆盖原生输出。
    pub output_bindings: BTreeMap<String, Expr>,
}
impl Node {
    /// 创建节点；默认无后继、无额外时限、无输出映射。
    pub fn new(id: impl Into<String>, action: Action) -> Self {
        Self {
            id: id.into(),
            next: None,
            timeout_ms: None,
            action,
            output_bindings: BTreeMap::new(),
        }
    }
    /// 设置作用域内后继。
    pub fn then(mut self, next: impl Into<String>) -> Self {
        self.next = Some(next.into());
        self
    }
}

/// 业务任务的准备边界。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// 注册模块拥有的稳定名称。
    pub type_id: String,
    /// 当前节点参数版本。
    pub version: u16,
    /// 仅在准备阶段解码的静态配置。
    pub config: serde_json::Value,
    /// 每次执行前求值的数据端口。
    pub inputs: BTreeMap<String, Expr>,
    /// 输入端口到词法资源名称。
    pub resources: BTreeMap<String, String>,
    /// 输出端口到当前作用域资源名称。
    pub resource_outputs: BTreeMap<String, String>,
    /// 明确启用的重试策略；None 为单次执行。
    pub retry: Option<Retry>,
}

/// 有界的任务重试；不能绕过任务的安全声明或副作用状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retry {
    /// 含首次执行在内的次数，1..=10。
    pub max_attempts: u32,
    /// 首次等待毫秒数。
    pub initial_delay_ms: u64,
    /// 指数退避的最大毫秒数。
    pub max_delay_ms: u64,
    /// 允许重试的错误类别。
    pub errors: Vec<ErrorKind>,
}

/// Switch 的一个值分支，不使用字符串隐式转换。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwitchCase {
    /// 必须与 selector 类型相同的常量。
    pub value: Value,
    /// 该分支拥有的子作用域。
    pub scope: String,
}

/// Try 的一个错误处理分支，按列表顺序匹配。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catch {
    /// 不重复的可捕获错误类型。
    pub errors: Vec<ErrorKind>,
    /// 错误处理子作用域。
    pub scope: String,
    /// 只读错误记录局部名称，不含外部原始错误数据。
    pub error_name: String,
}

/// 引擎内置控制结构；扩展模块只能添加 Task。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// 执行到此处才初始化变量。
    Let {
        /// 变量名。
        name: String,
        /// 固定类型。
        value_type: ValueType,
        /// 初值。
        value: Expr,
    },
    /// 所有右值成功后原子写入最近声明。
    Assign {
        /// 不重复的赋值目标。
        assignments: Vec<Assignment>,
    },
    /// 执行一个词法子作用域。
    Block {
        /// 独占子作用域。
        scope: String,
    },
    /// 执行恰好一个分支。
    If {
        /// 布尔条件。
        condition: Expr,
        /// 真分支作用域。
        then_scope: String,
        /// 假分支作用域，允许空块。
        else_scope: String,
    },
    /// 同类型值选择，无 fallthrough。
    Switch {
        /// 标量选择器。
        selector: Expr,
        /// 值分支。
        cases: Vec<SwitchCase>,
        /// 必须存在的默认分支。
        default_scope: String,
    },
    /// 每轮开始前检查条件。
    While {
        /// 布尔条件。
        condition: Expr,
        /// 每轮新建局部帧。
        body: String,
        /// 可覆盖默认轮数上限。
        max_iterations: Option<u32>,
    },
    /// 进入时冻结集合，串行执行每个元素。
    ForEach {
        /// 同类型列表。
        items: Expr,
        /// 只读元素名。
        item: String,
        /// 只读索引名。
        index: String,
        /// 每轮新建局部帧。
        body: String,
        /// 可覆盖默认轮数上限。
        max_iterations: Option<u32>,
    },
    /// 退出最近循环，不能穿过调用边界。
    Break,
    /// 继续最近循环。
    Continue,
    /// 有独立调用帧的子流程调用。
    Call {
        /// 子流程名称。
        subflow: String,
        /// 数据参数。
        inputs: BTreeMap<String, Expr>,
        /// 资源端口到词法资源名。
        resources: BTreeMap<String, String>,
    },
    /// 调用独立工作流；全局变量隔离，仅显式数据和资源参数可跨边界。
    CallWorkflow {
        /// 冻结 bundle 中的稳定工作流身份。
        workflow: crate::WorkflowId,
        /// 满足目标根输入声明的参数。
        inputs: BTreeMap<String, Expr>,
        /// 显式借用资源，所有权保留在调用方。
        resources: BTreeMap<String, String>,
    },
    /// 返回当前流程或子流程；嵌套块会先执行 Finally。
    Return {
        /// 满足声明的结果。
        values: BTreeMap<String, Expr>,
    },
    /// 结构化错误处理。
    Try {
        /// 尝试作用域。
        body: String,
        /// 错误分支。
        catches: Vec<Catch>,
        /// 总会执行的收尾作用域。
        finally: Option<String>,
    },
    /// 明确终止当前路径，可由 Catch 处理。
    Fail {
        /// 稳定用户错误代码，不求值用户文字。
        code: String,
    },
    /// 可取消等待。
    Wait {
        /// 非负整数毫秒数。
        milliseconds: Expr,
    },
    /// 已注册的业务任务。
    Task {
        /// 编译边界定义。
        task: Task,
    },
    /// 释放当前作用域拥有的资源；借用的祖先资源不可关闭。
    Release {
        /// 当前帧的资源名称。
        resource: String,
    },
}
