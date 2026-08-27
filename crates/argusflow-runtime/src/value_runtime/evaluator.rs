use rhai::{AST, Dynamic, Engine, EvalAltResult, Scope};
use serde_json::Value;

use super::{RuntimeValueScope, format_runtime_value, validate_json_pointer};
use crate::RuntimeError;

/// 使用同一安全配置编译仅包含表达式的 Rhai AST。
pub(crate) fn compile_expression(source: &str) -> Result<AST, String> {
    if source.trim().is_empty() {
        return Err("表达式不能为空".to_owned());
    }
    expression_engine()
        .compile_expression(source)
        .map_err(|error| error.to_string())
}

/// 在四个只读根对象组成的 Scope 中执行预编译 AST。
pub(crate) fn evaluate(
    ast: &AST,
    runtime_scope: &RuntimeValueScope,
) -> Result<Value, RuntimeError> {
    let mut scope = Scope::new();
    scope.push_constant(
        "input",
        to_dynamic(Value::Object(runtime_scope.input.clone()))?,
    );
    scope.push_constant(
        "vars",
        to_dynamic(Value::Object(runtime_scope.variables.clone()))?,
    );
    scope.push_constant(
        "nodes",
        to_dynamic(Value::Object(runtime_scope.nodes.clone()))?,
    );
    if let Some(result) = &runtime_scope.result {
        scope.push_constant("result", to_dynamic(Value::Object(result.clone()))?);
    }
    let dynamic = expression_engine()
        .eval_ast_with_scope::<Dynamic>(&mut scope, ast)
        .map_err(|error| RuntimeError::ExpressionEvaluation {
            message: error.to_string(),
        })?;
    rhai::serde::from_dynamic::<Value>(&dynamic.flatten()).map_err(|error| {
        RuntimeError::ExpressionResultNotJson {
            message: error.to_string(),
        }
    })
}

/// 创建不注册任何 I/O、进程、网络或资源能力的受限表达式引擎。
fn expression_engine() -> Engine {
    let mut engine = Engine::new();
    engine
        .disable_symbol("eval")
        .disable_symbol("print")
        .disable_symbol("debug")
        .set_max_operations(10_000)
        .set_max_expr_depths(48, 0)
        .set_max_call_levels(16)
        .set_max_string_size(1024 * 1024)
        .set_max_array_size(10_000)
        .set_max_map_size(10_000)
        .set_max_variables(16)
        .set_max_functions(0)
        .set_max_strings_interned(512)
        .set_allow_looping(false)
        .set_allow_anonymous_fn(false)
        .set_allow_statement_expression(false)
        .set_allow_loop_expressions(false)
        .set_allow_shadowing(false)
        .set_fail_on_invalid_map_property(true);
    engine.register_fn("str", runtime_str);
    engine.register_fn("json", runtime_json);
    engine.register_fn("get", runtime_get);
    engine
}

fn to_dynamic(value: Value) -> Result<Dynamic, RuntimeError> {
    rhai::serde::to_dynamic(value).map_err(|error| RuntimeError::ExpressionEvaluation {
        message: format!("JSON 值无法进入表达式作用域：{error}"),
    })
}

/// `str(value)` 使用和 Debug 节点一致的稳定文本格式。
fn runtime_str(value: Dynamic) -> Result<String, Box<EvalAltResult>> {
    let value = rhai::serde::from_dynamic::<Value>(&value.flatten())?;
    Ok(format_runtime_value(&value))
}

/// `json(value)` 生成紧凑且可再次解析的 JSON 文本。
fn runtime_json(value: Dynamic) -> Result<String, Box<EvalAltResult>> {
    let value = rhai::serde::from_dynamic::<Value>(&value.flatten())?;
    serde_json::to_string(&value).map_err(|error| error.to_string().into())
}

/// `get(value, pointer)` 以 RFC 6901 指针读取 JSON 子值。
fn runtime_get(value: Dynamic, pointer: &str) -> Result<Dynamic, Box<EvalAltResult>> {
    if !validate_json_pointer(pointer) {
        return Err(format!("JSON Pointer '{pointer}' 格式无效").into());
    }
    let value = rhai::serde::from_dynamic::<Value>(&value.flatten())?;
    let selected = value.pointer(pointer).cloned().ok_or_else(|| {
        Box::<EvalAltResult>::from(format!("JSON Pointer '{pointer}' 没有匹配到值"))
    })?;
    rhai::serde::to_dynamic(selected)
}

#[cfg(test)]
mod tests {
    use super::expression_engine;

    #[test]
    fn expression_engine_has_explicit_resource_limits() {
        let engine = expression_engine();
        assert_eq!(engine.max_operations(), 10_000);
        assert_eq!(engine.max_expr_depth(), 48);
        assert_eq!(engine.max_string_size(), 1024 * 1024);
        assert_eq!(engine.max_array_size(), 10_000);
        assert_eq!(engine.max_map_size(), 10_000);
        assert_eq!(engine.max_variables(), 16);
    }

    #[test]
    fn expression_engine_rejects_dynamic_eval_and_console_output() {
        let engine = expression_engine();
        assert!(engine.compile_expression("eval(\"40 + 2\")").is_err());
        assert!(engine.compile_expression("print(42)").is_err());
        assert!(engine.compile_expression("debug(42)").is_err());
    }
}
