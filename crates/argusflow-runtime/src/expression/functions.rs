//! 纯函数类型清单，与求值器穷尽对应。
use argusflow_workflow::{Function as F, ValueType as T};
pub(super) fn function_type(function: F, args: &[T]) -> Result<T, &'static str> {
    match (function, args) {
        (F::Length, [T::Text | T::List(_)]) => Ok(T::Int),
        (F::Concat, [T::Text, T::Text]) => Ok(T::Text),
        (F::Concat, [T::List(a), T::List(b)]) if a == b => Ok(T::List(a.clone())),
        (F::Contains, [T::Text, T::Text]) => Ok(T::Bool),
        (F::Contains, [T::List(a), b]) if a.as_ref() == b => Ok(T::Bool),
        (F::Trim | F::Lowercase | F::Uppercase, [T::Text]) => Ok(T::Text),
        (F::Split, [T::Text, T::Text]) => Ok(T::List(Box::new(T::Text))),
        (F::Join, [T::List(inner), T::Text]) if inner.as_ref() == &T::Text => Ok(T::Text),
        (F::Append, [T::List(a), b]) if a.as_ref() == b => Ok(T::List(a.clone())),
        (F::IsSome, [T::Optional(_)]) => Ok(T::Bool),
        (F::OrElse, [T::Optional(a), b]) if a.as_ref() == b => Ok(b.clone()),
        (F::ToFloat, [T::Int]) => Ok(T::Float),
        (F::ToText, [T::Int | T::Float | T::Bool | T::Text]) => Ok(T::Text),
        _ => Err("函数参数数量或类型不匹配"),
    }
}
