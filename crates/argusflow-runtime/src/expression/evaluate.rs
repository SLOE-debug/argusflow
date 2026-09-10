//! 求值只访问当前帧的词法链，不搜索调用者的局部帧。
use super::{
    ExprKind as K, PlanExpr,
    operations::{binary, function},
};
use crate::{RunError, compilation::Binding};
use argusflow_core::Operation;
use argusflow_workflow::{BinaryOp, ErrorKind, Function, Value, Values};

pub(crate) trait ValueReader {
    fn variable(&self, binding: Binding) -> Option<&Value>;
    fn input(&self, name: &str) -> Option<&Value>;
    fn output(&self, scope: usize, node: usize, name: &str) -> Option<&Value>;
}
pub(crate) struct Evaluation<'a> {
    pub reader: &'a dyn ValueReader,
    pub result: &'a Values,
    pub operation: &'a Operation,
    pub steps: usize,
    pub max_steps: usize,
    pub max_bytes: usize,
}
pub(crate) fn expression_error(message: &str) -> RunError {
    RunError::new(ErrorKind::Expression, message)
}
impl Evaluation<'_> {
    pub fn eval(&mut self, expression: &PlanExpr) -> Result<Value, RunError> {
        self.steps += 1;
        if self.steps > self.max_steps {
            return Err(RunError::new(ErrorKind::Limit, "表达式运算预算耗尽"));
        }
        self.operation
            .check("workflow_expression")
            .map_err(RunError::from)?;
        let unavailable =
            || RunError::new(ErrorKind::Contract, "已绑定值尚未初始化或不在当前激活帧中");
        let value = match &expression.kind {
            K::Literal(value) => value.clone(),
            K::Variable(binding) => self
                .reader
                .variable(*binding)
                .cloned()
                .ok_or_else(unavailable)?,
            K::Input(name) => self.reader.input(name).cloned().ok_or_else(unavailable)?,
            K::NodeOutput {
                scope,
                node,
                output,
            } => self
                .reader
                .output(*scope, *node, output)
                .cloned()
                .ok_or_else(unavailable)?,
            K::Result(name) => self.result.get(name).cloned().ok_or_else(unavailable)?,
            K::Field(source, field) => {
                let Value::Record(mut fields) = self.eval(source)? else {
                    return Err(unavailable());
                };
                fields.remove(field).ok_or_else(unavailable)?
            }
            K::Index(source, index) => {
                let Value::List(mut values) = self.eval(source)? else {
                    return Err(unavailable());
                };
                let Value::Int(index) = self.eval(index)? else {
                    return Err(unavailable());
                };
                let index =
                    usize::try_from(index).map_err(|_| expression_error("列表索引不能为负数"))?;
                if index >= values.len() {
                    return Err(expression_error("列表索引越界"));
                }
                values.swap_remove(index)
            }
            K::Binary(op, left, right) => {
                let left = self.eval(left)?;
                if *op == BinaryOp::And && left == Value::Bool(false) {
                    Value::Bool(false)
                } else if *op == BinaryOp::Or && left == Value::Bool(true) {
                    Value::Bool(true)
                } else {
                    binary(*op, left, self.eval(right)?)?
                }
            }
            K::Not(value) => {
                let Value::Bool(value) = self.eval(value)? else {
                    return Err(unavailable());
                };
                Value::Bool(!value)
            }
            K::Choose(condition, left, right) => {
                let Value::Bool(value) = self.eval(condition)? else {
                    return Err(unavailable());
                };
                self.eval(if value { left } else { right })?
            }
            K::Record(fields) => Value::Record(self.fields(fields)?),
            K::List(items) => {
                let mut values = Vec::new();
                let mut bytes = std::mem::size_of::<Value>();
                for expression in items {
                    let value = self.eval(expression)?;
                    self.charge(&mut bytes, &value, 0)?;
                    values.push(value);
                }
                Value::List(values)
            }
            K::Some(value) => Value::Optional(Some(Box::new(self.eval(value)?))),
            K::Function(Function::OrElse, arguments) => {
                let Value::Optional(value) = self.eval(&arguments[0])? else {
                    return Err(unavailable());
                };
                if let Some(value) = value {
                    *value
                } else {
                    self.eval(&arguments[1])?
                }
            }
            K::Function(f, args) => {
                let args = args
                    .iter()
                    .map(|e| self.eval(e))
                    .collect::<Result<Vec<_>, _>>()?;
                function(*f, args, self.max_bytes)?
            }
        };
        if value.measured_bytes().is_none_or(|n| n > self.max_bytes) {
            return Err(RunError::new(ErrorKind::Limit, "表达式值超过空间预算"));
        }
        if !value.matches(&expression.ty) {
            return Err(unavailable());
        }
        Ok(value)
    }
    pub fn fields(
        &mut self,
        fields: &std::collections::BTreeMap<String, PlanExpr>,
    ) -> Result<Values, RunError> {
        let mut values = Values::new();
        let mut bytes = std::mem::size_of::<Value>();
        for (name, expression) in fields {
            let value = self.eval(expression)?;
            self.charge(&mut bytes, &value, name.len())?;
            values.insert(name.clone(), value);
        }
        Ok(values)
    }
    fn charge(&self, bytes: &mut usize, value: &Value, extra: usize) -> Result<(), RunError> {
        *bytes = bytes
            .saturating_add(extra)
            .saturating_add(value.measured_bytes().unwrap_or(usize::MAX));
        if *bytes > self.max_bytes {
            return Err(RunError::new(
                ErrorKind::Limit,
                "表达式集合构造超过空间预算",
            ));
        }
        Ok(())
    }
}
