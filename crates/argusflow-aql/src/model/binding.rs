//! 编译产物可复用；每次绑定得到冻结且不可变的查询。
use super::{Attribute, Boundary, Condition, Expr, Operand, Value, ValueType};
use crate::{AqlError, DiagnosticCode};
use std::collections::{BTreeMap, BTreeSet};

/// 由调用方提供的参数值，不包含资源或执行逻辑。
pub type Bindings = BTreeMap<String, Value>;
/// 已解析并通过静态类型检查的英文查询。
#[derive(Debug, Clone)]
pub struct CompiledQuery {
    expression: Expr,
    parameters: BTreeMap<String, ValueType>,
}
impl CompiledQuery {
    pub(crate) fn new(expression: Expr, parameters: BTreeMap<String, ValueType>) -> Self {
        Self {
            expression,
            parameters,
        }
    }
    /// 只读查询结构。
    pub fn expression(&self) -> &Expr {
        &self.expression
    }
    /// 参数名称及其静态类型。
    pub fn parameters(&self) -> &BTreeMap<String, ValueType> {
        &self.parameters
    }
    /// 校验所有参数后冻结查询；拒绝缺失、多余、非有限数值和类型错误。
    pub fn bind(&self, bindings: &Bindings) -> Result<BoundQuery, AqlError> {
        if self.parameters.len() != bindings.len()
            || self.parameters.iter().any(|(name, kind)| {
                bindings
                    .get(name)
                    .is_none_or(|v| v.value_type() != *kind || !v.valid())
            })
        {
            return Err(AqlError::general(
                DiagnosticCode::Binding,
                "参数缺失、多余、类型不一致或值超过预算",
            ));
        }
        let mut expression = self.expression.clone();
        bind_expression(&mut expression, bindings);
        Ok(BoundQuery { expression })
    }
}
/// 参数已冻结的只读查询，后端只能接收此类型。
#[derive(Debug, Clone)]
pub struct BoundQuery {
    expression: Expr,
}
impl BoundQuery {
    /// 不可变执行结构。
    pub fn expression(&self) -> &Expr {
        &self.expression
    }
    /// 查询需要读取的属性。
    pub fn attributes(&self) -> BTreeSet<Attribute> {
        let mut attributes = BTreeSet::new();
        visit(&self.expression, &mut |expression| {
            if let Expr::Match {
                condition: Some(condition),
                ..
            } = expression
            {
                condition_attributes(condition, &mut attributes);
            }
        });
        attributes
    }
    /// 完整原生 CSS 字面量，用于后端预先建立成员集合。
    pub fn css_selectors(&self) -> BTreeSet<String> {
        let mut selectors = BTreeSet::new();
        visit(&self.expression, &mut |expression| {
            if let Expr::Css(selector) = expression {
                selectors.insert(selector.clone());
            }
        });
        selectors
    }
    /// 是否需要显式文档边界。
    pub fn uses_boundary(&self, expected: Boundary) -> bool {
        let mut found = false;
        visit(&self.expression, &mut |expression| {
            if let Expr::Enter { boundary, .. } = expression {
                found |= *boundary == expected;
            }
        });
        found
    }
}
pub(crate) fn visit(expression: &Expr, visitor: &mut impl FnMut(&Expr)) {
    visitor(expression);
    match expression {
        Expr::Relation { left, right, .. } => {
            visit(left, visitor);
            visit(right, visitor);
        }
        Expr::Nth { query, .. } => visit(query, visitor),
        Expr::Enter { host, .. } => visit(host, visitor),
        Expr::Match { .. } | Expr::Css(_) => {}
    }
}
fn condition_attributes(condition: &Condition, attributes: &mut BTreeSet<Attribute>) {
    match condition {
        Condition::Compare { attribute, .. } => {
            attributes.insert(*attribute);
        }
        Condition::And(left, right) | Condition::Or(left, right) => {
            condition_attributes(left, attributes);
            condition_attributes(right, attributes);
        }
        Condition::Not(inner) => condition_attributes(inner, attributes),
    }
}
fn bind_expression(expression: &mut Expr, bindings: &Bindings) {
    match expression {
        Expr::Match {
            condition: Some(condition),
            ..
        } => bind_condition(condition, bindings),
        Expr::Relation { left, right, .. } => {
            bind_expression(left, bindings);
            bind_expression(right, bindings);
        }
        Expr::Nth { query, .. } => bind_expression(query, bindings),
        Expr::Enter { host, .. } => bind_expression(host, bindings),
        Expr::Match {
            condition: None, ..
        }
        | Expr::Css(_) => {}
    }
}
fn bind_condition(condition: &mut Condition, bindings: &Bindings) {
    match condition {
        Condition::Compare { operand, .. } => {
            if let Operand::Parameter(name) = operand {
                // bind 已验证全部参数，使用显式分支避免在公共入口之后 panic。
                if let Some(value) = bindings.get(name) {
                    *operand = Operand::Literal(value.clone());
                }
            }
        }
        Condition::And(left, right) | Condition::Or(left, right) => {
            bind_condition(left, bindings);
            bind_condition(right, bindings);
        }
        Condition::Not(inner) => bind_condition(inner, bindings),
    }
}
