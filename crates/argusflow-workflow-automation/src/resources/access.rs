//! 冻结端口到具体能力对象的受检访问。
use argusflow_runtime::{RunError, TaskContext};
use argusflow_workflow::{ErrorKind, Value};

pub(crate) fn resource<'a, T: 'static>(
    context: &'a TaskContext<'_>,
    name: &str,
) -> Result<&'a T, RunError> {
    context
        .resources
        .get(name)
        .and_then(|resource| resource.as_any().downcast_ref::<T>())
        .ok_or_else(|| RunError::new(ErrorKind::Contract, "资源类型标识与适配器实现不一致"))
}
pub(crate) fn text<'a>(context: &'a TaskContext<'_>, name: &str) -> Result<&'a str, RunError> {
    match context.inputs.get(name) {
        Some(Value::Text(value)) => Ok(value),
        _ => Err(RunError::new(ErrorKind::Contract, "输入不是声明的文字类型")),
    }
}
