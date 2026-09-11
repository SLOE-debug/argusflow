//! 扁平作用域表与受控文档入口。
use crate::{Expr, Fields, Node};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 名称到资源类型的声明，不包含进程或平台句柄。
pub type ResourceFields = BTreeMap<String, String>;

/// 一个工作流以及随文档冻结的子流程定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    /// 唯一接受的文档版本为 1，无旧协议映射。
    pub schema_version: u32,
    /// 面向宿主的稳定工作流名称。
    pub name: String,
    /// 每次运行必须提供的只读数据输入。
    pub inputs: Fields,
    /// 根作用域正常结束及 Return 的结果类型。
    pub outputs: Fields,
    /// 宿主借给根作用域的资源，不转移所有权。
    pub resources: ResourceFields,
    /// 唯一根作用域 ID。
    pub root: String,
    /// 根、结构块和子流程作用域，ID 在文档内唯一。
    pub scopes: Vec<Scope>,
    /// 按名称调用的子流程清单。
    pub subflows: BTreeMap<String, Subflow>,
}

impl Workflow {
    /// 解析有大小上限的 JSON 文档；领域检查由 runtime::prepare 完成。
    pub fn from_json(source: &str) -> Result<Self, String> {
        if source.len() > 8 * 1024 * 1024 {
            return Err("工作流文档超过 8 MiB".into());
        }
        serde_json::from_str(source).map_err(|e| e.to_string())
    }
    /// 输出唯一当前版本文档。
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// 单个词法作用域；结构分支使用拥有的子作用域表达。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    /// 文档内唯一作用域 ID。
    pub id: String,
    /// 入口节点 ID；空作用域为 None。
    pub entry: Option<String>,
    /// 仅属于本作用域的节点；next 是作用域内有向边。
    pub nodes: Vec<Node>,
    /// 正常离开作用域时原子求值的公开输出。
    pub outputs: BTreeMap<String, Expr>,
}

/// 子流程具有根全局词法父级，不捕获调用点的局部帧。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subflow {
    /// 子流程入口作用域。
    pub scope: String,
    /// 每次调用冻结的数据参数。
    pub inputs: Fields,
    /// 每次调用显式借用的资源参数。
    pub resources: ResourceFields,
    /// 正常结束或 Return 必须满足的结果类型。
    pub outputs: Fields,
}

/// 可定位的准备阶段诊断。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// 独立工作流身份；单文档准备时未绑定。
    pub workflow: Option<crate::WorkflowId>,
    /// 稳定问题分类。
    pub code: DiagnosticCode,
    /// 不含运行输入的说明。
    pub message: String,
    /// 关联作用域。
    pub scope: Option<String>,
    /// 关联节点。
    pub node: Option<String>,
}

/// 编译失败的封闭类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    /// 文档或图结构不成立。
    Structure,
    /// 类型不相容。
    Type,
    /// 名称或输出不可见。
    Reference,
    /// 声明没有在所有到达路径初始化。
    Uninitialized,
    /// 资源类型、所有权或可见性无效。
    Resource,
    /// 控制转移越过合法边界。
    Control,
    /// 节点注册或参数编译失败。
    Task,
    /// 定义超过有限预算。
    Limit,
}
