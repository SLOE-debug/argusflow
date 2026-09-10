//! 表达式中的名字已经被绑定为槽位。
use crate::compilation::Binding;
use argusflow_workflow::{BinaryOp, Function, Value, ValueType};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(crate) struct PlanExpr {
    pub ty: ValueType,
    pub kind: ExprKind,
}
#[derive(Clone)]
pub(crate) enum ExprKind {
    Literal(Value),
    Variable(Binding),
    Input(String),
    NodeOutput {
        scope: usize,
        node: usize,
        output: String,
    },
    Result(String),
    Field(Box<PlanExpr>, String),
    Index(Box<PlanExpr>, Box<PlanExpr>),
    Binary(BinaryOp, Box<PlanExpr>, Box<PlanExpr>),
    Not(Box<PlanExpr>),
    Choose(Box<PlanExpr>, Box<PlanExpr>, Box<PlanExpr>),
    Record(BTreeMap<String, PlanExpr>),
    List(Vec<PlanExpr>),
    Some(Box<PlanExpr>),
    Function(Function, Vec<PlanExpr>),
}
