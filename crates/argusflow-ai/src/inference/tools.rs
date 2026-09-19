//! 模型工具只读冻结证据；不接受任意路径、脚本或网络地址。
use crate::{AiError, Evidence, Result};
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ids {
    ids: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    id: String,
}
pub(crate) fn definitions(vision: bool) -> Vec<Value> {
    let mut tools = vec![
        json!({"type":"function","function":{"name":"inspect_evidence","description":"根据时间线ID读取1到6条完整事实","parameters":{"type":"object","properties":{"ids":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":6}},"required":["ids"],"additionalProperties":false}}}),
    ];
    if vision {
        tools.push(json!({"type":"function","function":{"name":"view_change","description":"视觉记录ID的同来源前后局部变化图，最多三对","parameters":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false}}}));
    }
    tools
}
pub(crate) fn dispatch(
    evidence: &Evidence,
    call: &Value,
    vision: bool,
    pairs: &mut u8,
) -> Result<(Value, Vec<Value>, u64)> {
    let name = call["function"]["name"]
        .as_str()
        .ok_or_else(|| AiError::Invalid("工具名称无效".into()))?;
    let arguments = call["function"]["arguments"]
        .as_str()
        .filter(|s| s.len() <= 2048)
        .ok_or_else(|| AiError::Invalid("工具参数超限".into()))?;
    match name {
        "inspect_evidence" => Ok((
            evidence.inspect(&serde_json::from_str::<Ids>(arguments)?.ids)?,
            vec![],
            0,
        )),
        "view_change" if vision && *pairs < 3 => {
            *pairs += 1;
            let (images, pixels) =
                evidence.change(&serde_json::from_str::<Frame>(arguments)?.id)?;
            Ok((
                json!({"images":"随后 user 消息中的前后图片属于此次调用"}),
                images,
                pixels,
            ))
        }
        _ => Err(AiError::Invalid("工具不在允许范围内或图片预算耗尽".into())),
    }
}
