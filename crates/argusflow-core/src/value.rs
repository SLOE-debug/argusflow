use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 工作流节点参数可引用的运行时值表达式。
///
/// 表达式只描述数据来源，不在核心层执行；Runtime 会在节点准备阶段把它解析成冻结值。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValueExpr {
    /// 直接嵌入工作流定义的 JSON 值。
    Literal {
        /// 不依赖运行状态的常量值。
        value: Value,
    },
    /// 读取本次运行启动时提供的输入字段。
    WorkflowInput {
        /// 输入对象中的一级字段名。
        key: String,
    },
    /// 读取一个已经成功执行节点的值输出端口。
    NodeOutput {
        /// 产生值的节点 ID。
        node_id: String,
        /// 节点公开的值输出端口名称。
        output: String,
    },
    /// 读取本次运行的可变变量存储。
    Variable {
        /// 变量存储中的一级字段名。
        name: String,
    },
}

impl ValueExpr {
    /// 创建字符串字面量，供工作流构造器避免重复拼装 JSON。
    pub fn text(value: impl Into<String>) -> Self {
        Self::Literal {
            value: Value::String(value.into()),
        }
    }
}
