use serde_json::Value;

/// 将任意 JSON 值稳定格式化为 Debug 和纯表达式 helper 使用的文本。
pub(crate) fn format_runtime_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::format_runtime_value;

    #[test]
    fn formats_every_json_category_without_losing_structure() {
        assert_eq!(format_runtime_value(&json!("plain")), "plain");
        assert_eq!(format_runtime_value(&json!(42)), "42");
        assert_eq!(format_runtime_value(&json!(true)), "true");
        assert_eq!(format_runtime_value(&json!(null)), "null");
        assert_eq!(format_runtime_value(&json!([1, 2])), "[\n  1,\n  2\n]");
        assert_eq!(
            format_runtime_value(&json!({ "nested": { "ok": true } })),
            "{\n  \"nested\": {\n    \"ok\": true\n  }\n}"
        );
    }
}
