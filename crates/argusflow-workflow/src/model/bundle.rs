//! 独立工作流的稳定身份和一次运行的冻结依赖。
use crate::Workflow;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 不依赖文件路径或显示名称的工作流身份。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkflowId(pub String);

/// 宿主在运行前读取的完整快照；运行中不再读取文件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBundle {
    /// 本次执行的入口文档身份。
    pub root: WorkflowId,
    /// 按身份索引的独立定义。
    pub workflows: BTreeMap<WorkflowId, Workflow>,
}
