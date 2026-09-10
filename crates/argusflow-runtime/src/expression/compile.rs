//! 所有符号解析与运算类型检查在执行之前完成。
use super::{ExprKind as K, PlanExpr, functions::function_type};
use crate::compilation::{Binding, Environment, Slot};
use argusflow_workflow::{BinaryOp as Op, DiagnosticCode as Code, Expr, Fields, ValueType as Ty};
use std::collections::{BTreeMap, BTreeSet};

type Error = (Code, String);
pub(crate) fn compile_expr(
    source: &Expr,
    env: &Environment,
    symbols: &[BTreeMap<String, Binding>],
    slots: &[Vec<Slot>],
    result: &Fields,
) -> Result<(PlanExpr, BTreeSet<Binding>), Error> {
    let mut compiler = ExpressionCompiler {
        env,
        symbols,
        slots,
        result,
        reads: BTreeSet::new(),
        count: 0,
    };
    let plan = compiler.compile(source, 0)?;
    Ok((plan, compiler.reads))
}
struct ExpressionCompiler<'a> {
    env: &'a Environment,
    symbols: &'a [BTreeMap<String, Binding>],
    slots: &'a [Vec<Slot>],
    result: &'a Fields,
    reads: BTreeSet<Binding>,
    count: usize,
}
fn error(code: Code, message: &str) -> Error {
    (code, message.to_owned())
}
impl ExpressionCompiler<'_> {
    fn compile(&mut self, source: &Expr, depth: usize) -> Result<PlanExpr, Error> {
        self.count += 1;
        if depth > 48 || self.count > 4096 {
            return Err(error(Code::Limit, "表达式深度或节点数超过预算"));
        }
        let (ty, kind) =
            match source {
                Expr::Literal { value_type, value } => {
                    if !value_type.is_valid()
                        || !value.matches(value_type)
                        || value.measured_bytes().is_none_or(|size| size > 1024 * 1024)
                    {
                        return Err(error(Code::Type, "字面量类型无效或超过 1 MiB"));
                    }
                    (value_type.clone(), K::Literal(value.clone()))
                }
                Expr::Variable { name } => {
                    let binding = self
                        .env
                        .chain
                        .iter()
                        .rev()
                        .find_map(|scope| self.symbols[*scope].get(name))
                        .copied()
                        .ok_or_else(|| error(Code::Reference, "变量未声明"))?;
                    if !self.env.initialized.contains(&binding) {
                        return Err(error(
                            Code::Uninitialized,
                            "变量尚未初始化；内层同名声明不会回退到外层",
                        ));
                    }
                    self.reads.insert(binding);
                    (
                        self.slots[binding.scope][binding.slot].ty.clone(),
                        K::Variable(binding),
                    )
                }
                Expr::Input { name } => (
                    self.env
                        .inputs
                        .get(name)
                        .cloned()
                        .ok_or_else(|| error(Code::Reference, "输入或子流程参数不存在"))?,
                    K::Input(name.clone()),
                ),
                Expr::NodeOutput { node, output } => {
                    let (scope, node, fields) =
                        self.env.outputs.get(node).ok_or_else(|| {
                            error(Code::Reference, "节点结果不在可见的已完成路径中")
                        })?;
                    (
                        fields
                            .get(output)
                            .cloned()
                            .ok_or_else(|| error(Code::Reference, "节点输出不存在"))?,
                        K::NodeOutput {
                            scope: *scope,
                            node: *node,
                            output: output.clone(),
                        },
                    )
                }
                Expr::Result { output } => (
                    self.result.get(output).cloned().ok_or_else(|| {
                        error(Code::Reference, "原生输出仅在当前节点输出映射中可见")
                    })?,
                    K::Result(output.clone()),
                ),
                Expr::Field { value, field } => {
                    let value = self.compile(value, depth + 1)?;
                    let Ty::Record(fields) = &value.ty else {
                        return Err(error(Code::Type, "字段访问需要记录"));
                    };
                    (
                        fields
                            .get(field)
                            .cloned()
                            .ok_or_else(|| error(Code::Reference, "记录字段不存在"))?,
                        K::Field(Box::new(value), field.clone()),
                    )
                }
                Expr::Index { value, index } => {
                    let value = self.compile(value, depth + 1)?;
                    let index = self.compile(index, depth + 1)?;
                    let Ty::List(item) = &value.ty else {
                        return Err(error(Code::Type, "下标访问需要列表"));
                    };
                    if index.ty != Ty::Int {
                        return Err(error(Code::Type, "列表下标必须为整数"));
                    }
                    (*item.clone(), K::Index(Box::new(value), Box::new(index)))
                }
                Expr::Binary { op, left, right } => {
                    let left = self.compile(left, depth + 1)?;
                    let right = self.compile(right, depth + 1)?;
                    let ty = binary_type(*op, &left.ty, &right.ty)?;
                    (ty, K::Binary(*op, Box::new(left), Box::new(right)))
                }
                Expr::Not { value } => {
                    let value = self.compile(value, depth + 1)?;
                    if value.ty != Ty::Bool {
                        return Err(error(Code::Type, "Not 要求布尔值"));
                    }
                    (Ty::Bool, K::Not(Box::new(value)))
                }
                Expr::Choose {
                    condition,
                    then_value,
                    else_value,
                } => {
                    let condition = self.compile(condition, depth + 1)?;
                    let left = self.compile(then_value, depth + 1)?;
                    let right = self.compile(else_value, depth + 1)?;
                    if condition.ty != Ty::Bool || left.ty != right.ty {
                        return Err(error(Code::Type, "条件选择需要布尔条件及相同分支类型"));
                    }
                    (
                        left.ty.clone(),
                        K::Choose(Box::new(condition), Box::new(left), Box::new(right)),
                    )
                }
                Expr::Record { fields } => {
                    let compiled = fields
                        .iter()
                        .map(|(name, e)| Ok((name.clone(), self.compile(e, depth + 1)?)))
                        .collect::<Result<BTreeMap<_, _>, Error>>()?;
                    let ty = Ty::Record(
                        compiled
                            .iter()
                            .map(|(name, e)| (name.clone(), e.ty.clone()))
                            .collect(),
                    );
                    (ty, K::Record(compiled))
                }
                Expr::List { item_type, items } => {
                    if !item_type.is_valid() {
                        return Err(error(Code::Type, "列表元素类型声明超出预算"));
                    }
                    let compiled = items
                        .iter()
                        .map(|e| self.compile(e, depth + 1))
                        .collect::<Result<Vec<_>, _>>()?;
                    if compiled.iter().any(|e| &e.ty != item_type) {
                        return Err(error(Code::Type, "列表元素类型不同"));
                    }
                    (Ty::List(Box::new(item_type.clone())), K::List(compiled))
                }
                Expr::Some { value } => {
                    let value = self.compile(value, depth + 1)?;
                    (
                        Ty::Optional(Box::new(value.ty.clone())),
                        K::Some(Box::new(value)),
                    )
                }
                Expr::Function {
                    function,
                    arguments,
                } => {
                    let arguments = arguments
                        .iter()
                        .map(|e| self.compile(e, depth + 1))
                        .collect::<Result<Vec<_>, _>>()?;
                    let types = arguments.iter().map(|e| e.ty.clone()).collect::<Vec<_>>();
                    (
                        function_type(*function, &types).map_err(|msg| error(Code::Type, msg))?,
                        K::Function(*function, arguments),
                    )
                }
            };
        if !ty.is_valid() {
            return Err(error(Code::Type, "表达式结果类型结构或字段名称无效"));
        }
        Ok(PlanExpr { ty, kind })
    }
}

fn binary_type(op: Op, left: &Ty, right: &Ty) -> Result<Ty, Error> {
    if left != right {
        return Err(error(Code::Type, "二元运算不执行隐式类型转换"));
    }
    let numeric = matches!(left, Ty::Int | Ty::Float);
    match op {
        Op::Equal | Op::NotEqual => Ok(Ty::Bool),
        Op::And | Op::Or if left == &Ty::Bool => Ok(Ty::Bool),
        Op::Less | Op::LessEqual | Op::Greater | Op::GreaterEqual
            if numeric || left == &Ty::Text =>
        {
            Ok(Ty::Bool)
        }
        Op::Add | Op::Subtract | Op::Multiply | Op::Divide if numeric => Ok(left.clone()),
        Op::Remainder if left == &Ty::Int => Ok(Ty::Int),
        _ => Err(error(Code::Type, "运算不支持当前类型")),
    }
}
