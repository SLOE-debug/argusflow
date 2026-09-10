//! 冻结的执行计划与编译器内部符号。
use crate::{PreparedTask, TaskSignature, expression::PlanExpr};
use argusflow_workflow::{ErrorKind, Fields, Retry, Value, ValueType};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// 可被多次运行共享的只读计划；只有 prepare 可以构造。
pub struct PreparedWorkflow {
    pub(crate) root: usize,
    pub(crate) scopes: Vec<PlanScope>,
    pub(crate) inputs: Fields,
    pub(crate) resource_inputs: BTreeMap<String, String>,
    pub(crate) name: String,
}
impl PreparedWorkflow {
    /// 文档名称。
    pub fn name(&self) -> &str {
        &self.name
    }
    /// 根输出类型。
    pub fn outputs(&self) -> Fields {
        self.scopes[self.root]
            .outputs
            .iter()
            .map(|(k, e)| (k.clone(), e.ty.clone()))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Binding {
    pub scope: usize,
    pub slot: usize,
}
#[derive(Clone)]
pub(crate) struct Slot {
    pub ty: ValueType,
    pub mutable: bool,
}
pub(crate) struct PlanScope {
    pub can_complete: bool,
    pub id: String,
    pub nodes: Vec<PlanNode>,
    pub slots: Vec<Slot>,
    pub outputs: BTreeMap<String, PlanExpr>,
}
pub(crate) struct PlanNode {
    pub id: String,
    pub timeout_ms: Option<u64>,
    pub action: PlanAction,
    pub mappings: BTreeMap<String, PlanExpr>,
    pub output_types: Fields,
}
pub(crate) struct PlanTask {
    pub task: Arc<dyn PreparedTask>,
    pub signature: TaskSignature,
    pub inputs: BTreeMap<String, PlanExpr>,
    pub resources: BTreeMap<String, ResourceBinding>,
    pub resource_outputs: BTreeMap<String, String>,
    pub retry: Option<Retry>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ResourceBinding {
    pub scope: usize,
    pub name: String,
}
pub(crate) struct PlanCatch {
    pub errors: Vec<ErrorKind>,
    pub scope: usize,
    pub error_slot: usize,
}
pub(crate) enum PlanAction {
    Let {
        slot: usize,
        value: PlanExpr,
    },
    Assign(Vec<(Binding, PlanExpr)>),
    Block(usize),
    If {
        condition: PlanExpr,
        then_scope: usize,
        else_scope: usize,
    },
    Switch {
        selector: PlanExpr,
        cases: Vec<(Value, usize)>,
        default_scope: usize,
    },
    While {
        condition: PlanExpr,
        body: usize,
        max_iterations: Option<u32>,
    },
    ForEach {
        items: PlanExpr,
        item_slot: usize,
        index_slot: usize,
        body: usize,
        max_iterations: Option<u32>,
    },
    Break,
    Continue,
    Call {
        scope: usize,
        inputs: BTreeMap<String, PlanExpr>,
        resources: BTreeMap<String, ResourceBinding>,
    },
    Return(BTreeMap<String, PlanExpr>),
    Try {
        body: usize,
        catches: Vec<PlanCatch>,
        finally: Option<usize>,
    },
    Fail(String),
    Wait(PlanExpr),
    Task(PlanTask),
    Release(ResourceBinding),
}

#[derive(Clone, Default)]
pub(crate) struct Environment {
    pub chain: Vec<usize>,
    pub initialized: BTreeSet<Binding>,
    pub outputs: BTreeMap<String, (usize, usize, Fields)>,
    pub resources: BTreeMap<String, (ResourceBinding, String, bool)>,
    pub inputs: Fields,
    pub return_types: Option<Fields>,
    pub loop_depth: usize,
    pub in_finally: bool,
}
