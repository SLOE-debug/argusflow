//! 检查溢出和结果分配规模的纯运算。
use super::evaluate::expression_error;
use crate::RunError;
use argusflow_workflow::{BinaryOp as O, ErrorKind, Function as F, Value as V};

pub(super) fn binary(op: O, left: V, right: V) -> Result<V, RunError> {
    if op == O::Equal {
        return Ok(V::Bool(left == right));
    }
    if op == O::NotEqual {
        return Ok(V::Bool(left != right));
    }
    let order = match (&left, &right) {
        (V::Int(a), V::Int(b)) => a.partial_cmp(b),
        (V::Float(a), V::Float(b)) => a.partial_cmp(b),
        (V::Text(a), V::Text(b)) => a.partial_cmp(b),
        _ => None,
    };
    if let Some(order) = order {
        let compared = match op {
            O::Less => Some(order.is_lt()),
            O::LessEqual => Some(order.is_le()),
            O::Greater => Some(order.is_gt()),
            O::GreaterEqual => Some(order.is_ge()),
            _ => None,
        };
        if let Some(value) = compared {
            return Ok(V::Bool(value));
        }
    }
    match (left, right) {
        (V::Int(a), V::Int(b)) => {
            let value = match op {
                O::Add => a.checked_add(b),
                O::Subtract => a.checked_sub(b),
                O::Multiply => a.checked_mul(b),
                O::Divide => a.checked_div(b),
                O::Remainder => a.checked_rem(b),
                _ => None,
            };
            value
                .map(V::Int)
                .ok_or_else(|| expression_error("整数运算溢出或除零"))
        }
        (V::Float(a), V::Float(b)) => {
            let value = match op {
                O::Add => a + b,
                O::Subtract => a - b,
                O::Multiply => a * b,
                O::Divide => a / b,
                _ => f64::NAN,
            };
            if value.is_finite() {
                Ok(V::Float(value))
            } else {
                Err(expression_error("浮点运算结果不是有限数"))
            }
        }
        (V::Bool(a), V::Bool(b)) => match op {
            O::And => Ok(V::Bool(a && b)),
            O::Or => Ok(V::Bool(a || b)),
            _ => Err(expression_error("无效布尔运算")),
        },
        _ => Err(expression_error("运算类型不匹配")),
    }
}

pub(super) fn function(f: F, args: Vec<V>, max_bytes: usize) -> Result<V, RunError> {
    let limit = || RunError::new(ErrorKind::Limit, "集合或文字结果超过空间预算");
    match (f, args.as_slice()) {
        (F::Length, [V::Text(v)]) => Ok(V::Int(v.chars().count() as i64)),
        (F::Length, [V::List(v)]) => Ok(V::Int(v.len() as i64)),
        (F::Concat, [V::Text(a), V::Text(b)]) => {
            if a.len().saturating_add(b.len()) > max_bytes {
                return Err(limit());
            }
            Ok(V::Text(format!("{a}{b}")))
        }
        (F::Concat, [V::List(a), V::List(b)]) => {
            if a.len()
                .saturating_add(b.len())
                .saturating_mul(std::mem::size_of::<V>())
                > max_bytes
            {
                return Err(limit());
            }
            Ok(V::List(a.iter().chain(b).cloned().collect()))
        }
        (F::Contains, [V::Text(a), V::Text(b)]) => Ok(V::Bool(a.contains(b))),
        (F::Contains, [V::List(a), b]) => Ok(V::Bool(a.contains(b))),
        (F::Trim, [V::Text(v)]) => Ok(V::Text(v.trim().into())),
        (F::Lowercase, [V::Text(v)]) => Ok(V::Text(v.to_lowercase())),
        (F::Uppercase, [V::Text(v)]) => Ok(V::Text(v.to_uppercase())),
        (F::Split, [V::Text(value), V::Text(separator)]) => {
            if separator.is_empty() {
                return Err(expression_error("Split 分隔符不能为空"));
            }
            let mut bytes = 0usize;
            let mut parts = Vec::new();
            for part in value.split(separator) {
                bytes = bytes
                    .saturating_add(std::mem::size_of::<V>())
                    .saturating_add(part.len());
                if bytes > max_bytes {
                    return Err(limit());
                }
                parts.push(V::Text(part.into()));
            }
            Ok(V::List(parts))
        }
        (F::Join, [V::List(values), V::Text(separator)]) => {
            let parts = values
                .iter()
                .map(|v| {
                    if let V::Text(s) = v {
                        Ok(s.as_str())
                    } else {
                        Err(expression_error("Join 元素不是文字"))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let size = parts.iter().map(|s| s.len()).sum::<usize>().saturating_add(
                separator
                    .len()
                    .saturating_mul(parts.len().saturating_sub(1)),
            );
            if size > max_bytes {
                return Err(limit());
            }
            Ok(V::Text(parts.join(separator)))
        }
        (F::Append, [V::List(values), item]) => {
            let mut result = values.clone();
            result.push(item.clone());
            Ok(V::List(result))
        }
        (F::IsSome, [V::Optional(value)]) => Ok(V::Bool(value.is_some())),
        (F::ToFloat, [V::Int(value)]) if value.unsigned_abs() <= (1u64 << 53) => {
            Ok(V::Float(*value as f64))
        }
        (F::ToFloat, [V::Int(_)]) => Err(expression_error("整数不能精确表示为浮点数")),
        (F::ToText, [V::Int(v)]) => Ok(V::Text(v.to_string())),
        (F::ToText, [V::Float(v)]) => Ok(V::Text(v.to_string())),
        (F::ToText, [V::Bool(v)]) => Ok(V::Text(v.to_string())),
        (F::ToText, [V::Text(v)]) => Ok(V::Text(v.clone())),
        _ => Err(expression_error("纯函数参数不满足冻结契约")),
    }
}
