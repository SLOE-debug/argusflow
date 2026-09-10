//! 强三值属性判断，缺失值不会因为 not 或 != 成为肯定匹配。
use crate::{Attribute, Condition, MatchOperator, Operand, Value};
use std::collections::BTreeMap;

pub(super) fn matches(condition: &Condition, values: &BTreeMap<Attribute, Value>) -> Option<bool> {
    match condition {
        Condition::Not(inner) => matches(inner, values).map(|value| !value),
        Condition::And(left, right) => match (matches(left, values), matches(right, values)) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        },
        Condition::Or(left, right) => match (matches(left, values), matches(right, values)) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        Condition::Compare {
            attribute,
            operator,
            operand,
        } => {
            let left = values.get(attribute)?;
            if let (Value::Text(text), Operand::Regex { compiled, .. }) = (left, operand) {
                return Some(compiled.is_match(text));
            }
            let Operand::Literal(right) = operand else {
                return None;
            };
            Some(match operator {
                MatchOperator::Equal => left == right,
                MatchOperator::NotEqual => left != right,
                MatchOperator::Contains => text_compare(left, right, |a, b| a.contains(b))?,
                MatchOperator::StartsWith => text_compare(left, right, |a, b| a.starts_with(b))?,
                MatchOperator::EndsWith => text_compare(left, right, |a, b| a.ends_with(b))?,
                MatchOperator::Greater => number_compare(left, right, |a, b| a > b)?,
                MatchOperator::GreaterEqual => number_compare(left, right, |a, b| a >= b)?,
                MatchOperator::Less => number_compare(left, right, |a, b| a < b)?,
                MatchOperator::LessEqual => number_compare(left, right, |a, b| a <= b)?,
                MatchOperator::Regex => return None,
            })
        }
    }
}
fn text_compare(
    left: &Value,
    right: &Value,
    compare: impl FnOnce(&str, &str) -> bool,
) -> Option<bool> {
    if let (Value::Text(left), Value::Text(right)) = (left, right) {
        Some(compare(left, right))
    } else {
        None
    }
}
fn number_compare(
    left: &Value,
    right: &Value,
    compare: impl FnOnce(f64, f64) -> bool,
) -> Option<bool> {
    if let (Value::Number(left), Value::Number(right)) = (left, right) {
        Some(compare(*left, *right))
    } else {
        None
    }
}
