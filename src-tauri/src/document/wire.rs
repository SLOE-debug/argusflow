//! 在唯一 IPC 边界无损转换 i64/u64，领域 JSON 协议不改变。
use argusflow_workflow::Workflow;
use serde_json::{Map, Value};

#[cfg(test)]
#[path = "../../../tests/argusflow-desktop/unit/wire.rs"]
mod tests;

/// 前端定义转换为当前 Rust 契约并检查预算。
pub fn decode_workflow(value: &Value) -> Result<Workflow, String> {
    let mut value = value.clone();
    if let Some(scopes) = value.get_mut("scopes").and_then(Value::as_array_mut) {
        for scope in scopes {
            if let Some(nodes) = scope.get_mut("nodes").and_then(Value::as_array_mut) {
                for node in nodes {
                    let object = node.as_object_mut().ok_or("节点必须为对象")?;
                    if object
                        .get("timeout_ms")
                        .is_some_and(|value| !value.is_null())
                    {
                        convert_integer(object, "timeout_ms", false, false)?;
                    }
                    if let Some(task) = object
                        .get_mut("action")
                        .and_then(|action| action.get_mut("task"))
                    {
                        let id = task
                            .get("type_id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        if let Some(config) = task.get_mut("config") {
                            decode_config(&id, config)?;
                        }
                    }
                }
            }
        }
    }
    convert(&mut value, false, 0)?;
    let source = serde_json::to_string(&value).map_err(|e| e.to_string())?;
    Workflow::from_json(&source)
}
/// 将最终数据编码成前端无损值。
pub fn encode_values(value: &argusflow_workflow::Values) -> Result<Value, String> {
    let mut value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    convert(&mut value, true, 0)?;
    Ok(value)
}
/// 运行输入使用同一无损整数规则。
pub fn decode_inputs(mut value: Value) -> Result<argusflow_workflow::Values, String> {
    convert(&mut value, false, 0)?;
    serde_json::from_value(value).map_err(|e| e.to_string())
}
fn convert(value: &mut Value, encode: bool, depth: usize) -> Result<(), String> {
    if depth > 128 {
        return Err("传输数据层级过深".into());
    }
    match value {
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("int")
                && object.contains_key("value")
            {
                convert_integer(object, "value", encode, true)?;
            }
            let is_task = object.get("type_id").is_some_and(Value::is_string);
            for (key, child) in object.iter_mut() {
                if key == "config" && is_task && !encode {
                    continue;
                }
                convert(child, encode, depth + 1)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                convert(child, encode, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
/// 已注册动作配置中的 u64 字段在同一边界显式解码。
pub fn decode_config(type_id: &str, config: &mut Value) -> Result<(), String> {
    if type_id == "aql.wait" {
        let object = config.as_object_mut().ok_or("节点配置必须为对象")?;
        if object.contains_key("interval_ms") {
            convert_integer(object, "interval_ms", false, false)?;
        }
    }
    Ok(())
}
fn convert_integer(
    object: &mut Map<String, Value>,
    key: &str,
    encode: bool,
    signed: bool,
) -> Result<(), String> {
    let value = &object[key];
    let next = if encode {
        Value::String(value.as_number().ok_or("整数编码类型错误")?.to_string())
    } else {
        let text = value.as_str().ok_or("整数必须通过十进制字符串传输")?;
        if signed {
            Value::from(text.parse::<i64>().map_err(|_| "整数超出 i64 范围")?)
        } else {
            Value::from(text.parse::<u64>().map_err(|_| "整数超出 u64 范围")?)
        }
    };
    object.insert(key.into(), next);
    Ok(())
}
