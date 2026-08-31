//! AQL v3 观察表达式的稳定格式化与 canonical identity。

use argusflow_core::{
    EntityField, NumberComparison, NumberOperand, ObservationExpr, ObservationQuery,
};

use crate::{canonicalize_query, format_query};

/// 输出无无关空白、可用作缓存键的 AQL v3 观察表达式。
pub fn canonicalize_observation(query: &ObservationQuery) -> String {
    format_expression(&query.expression, true)
}

/// 输出适合编辑器展示的 AQL v3 观察表达式。
pub fn format_observation(query: &ObservationQuery) -> String {
    format_expression(&query.expression, false)
}

/// 递归格式化表达式；selector 的规范化继续由既有格式器负责。
fn format_expression(expression: &ObservationExpr, compact: bool) -> String {
    match expression {
        ObservationExpr::Entities { query } => selector(query, compact),
        ObservationExpr::Project { query, fields } => {
            let separator = if compact { "," } else { ", " };
            let assignment = if compact { "=" } else { " = " };
            let fields = fields
                .iter()
                .map(|field| field_name(*field))
                .collect::<Vec<_>>()
                .join(separator);
            format!(
                "project({}{separator}fields{assignment}[{fields}])",
                selector(query, compact),
            )
        }
        ObservationExpr::Count { query } => format!("count({})", selector(query, compact)),
        ObservationExpr::Exists { query } => format!("exists({})", selector(query, compact)),
        ObservationExpr::Compare {
            left,
            operator,
            right,
        } => {
            let spacing = if compact { "" } else { " " };
            format!(
                "{}{spacing}{}{spacing}{}",
                format_expression(left, compact),
                comparison(*operator),
                operand(right),
            )
        }
        ObservationExpr::AllOf { expressions } => list("all_of", expressions, compact),
        ObservationExpr::AnyOf { expressions } => list("any_of", expressions, compact),
        ObservationExpr::Not { expression } => {
            format!("not({})", format_expression(expression, compact))
        }
    }
}

/// 格式化一个布尔表达式列表。
fn list(name: &str, expressions: &[ObservationExpr], compact: bool) -> String {
    let separator = if compact { "," } else { ", " };
    format!(
        "{name}({})",
        expressions
            .iter()
            .map(|expression| format_expression(expression, compact))
            .collect::<Vec<_>>()
            .join(separator),
    )
}

/// 选择 canonical 或可读 selector 格式。
fn selector(query: &argusflow_core::UiQuery, compact: bool) -> String {
    if compact {
        canonicalize_query(query)
    } else {
        format_query(query)
    }
}

/// 返回字段的稳定 AQL 拼写。
const fn field_name(field: EntityField) -> &'static str {
    match field {
        EntityField::Name => "name",
        EntityField::Text => "text",
        EntityField::Value => "value",
        EntityField::Role => "role",
        EntityField::Bounds => "bounds",
        EntityField::Confidence => "confidence",
        EntityField::Source => "source",
    }
}

/// 返回数量比较符号。
const fn comparison(operator: NumberComparison) -> &'static str {
    match operator {
        NumberComparison::Equal => "=",
        NumberComparison::NotEqual => "!=",
        NumberComparison::GreaterThan => ">",
        NumberComparison::GreaterThanOrEqual => ">=",
        NumberComparison::LessThan => "<",
        NumberComparison::LessThanOrEqual => "<=",
    }
}

/// 输出冻结整数或参数引用。
fn operand(operand: &NumberOperand) -> String {
    match operand {
        NumberOperand::Literal(value) => value.to_string(),
        NumberOperand::Parameter(name) => format!("${name}"),
    }
}
