use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// 工作流声明的单个运行时输入。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowInputDefinition {
    /// 工作流内唯一的一级字段名。
    pub key: String,
    /// 输入值必须满足的稳定类型约束。
    pub value_type: WorkflowInputType,
}

/// 当前工作流值表达式可以消费的运行时输入类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowInputType {
    /// 不执行隐式转换的 UTF-8 文本值。
    Text,
}

/// 调用方为一次工作流运行提供的瞬时输入。
///
/// 该对象不属于持久化工作流定义，并在运行启动时冻结。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunInputs {
    /// 按工作流输入声明命名的 JSON 值。
    pub values: Map<String, Value>,
}
