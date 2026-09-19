use super::{model::*, prompt, tools, validation};
use crate::{
    AiConfig, AiError, Evidence, Result,
    transport::{Client, Transport},
};
use argusflow_runtime::NodeRegistry;
use serde_json::json;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
/// 在同一对话中补读事实并反馈编译错误；仅返回草案，绝不执行输出。
pub async fn analyze(
    config: AiConfig,
    key: String,
    evidence: Arc<Evidence>,
    registry: &NodeRegistry,
    cancel: CancellationToken,
    progress: impl Fn(Progress),
) -> Result<InferenceResult> {
    let client = Client::new(config.clone(), key)?;
    run(config, client, evidence, registry, cancel, progress).await
}
pub(super) async fn run(
    config: AiConfig,
    client: impl Transport,
    evidence: Arc<Evidence>,
    registry: &NodeRegistry,
    cancel: CancellationToken,
    progress: impl Fn(Progress),
) -> Result<InferenceResult> {
    if cancel.is_cancelled() {
        return Err(AiError::Cancelled);
    }
    let mut initial = vec![json!({"type":"text","text":evidence.timeline()?.to_string()})];
    let mut retained_pixels = 0;
    if config.vision {
        let evidence = evidence.clone();
        let (blocks, pixels) = tokio::task::spawn_blocking(move || evidence.overview())
            .await
            .map_err(|_| AiError::Invalid("图片任务中断".into()))??;
        initial.extend(blocks);
        retained_pixels = pixels;
    }
    let mut messages = vec![
        json!({"role":"system","content":prompt::system(registry)}),
        json!({"role":"user","content":initial}),
    ];
    let mut metrics = Metrics::default();
    let mut pairs = 0;
    for round in 1..=config.max_rounds {
        if cancel.is_cancelled() {
            return Err(AiError::Cancelled);
        }
        progress(Progress {
            round,
            stage: ProgressStage::Analyzing,
        });
        let allow_tools = round < config.max_rounds && metrics.tool_calls < config.max_tools;
        if !allow_tools {
            messages.push(json!({"role":"user","content":"本轮输出最终严格 JSON；无法确定的操作保留在 unresolved，不能编造。"}));
        }
        metrics.image_pixels += retained_pixels;
        if metrics.image_pixels > 5_000_000 {
            return Err(AiError::Budget("累计图片超过 500 万像素".into()));
        }
        // 去除图片载荷后单独检查文本预算，历史工具结果也计入。
        let mut text_only = messages.clone();
        for m in &mut text_only {
            if let Some(parts) = m["content"].as_array_mut() {
                for p in parts {
                    if p["type"] == "image_url" {
                        *p = json!({"type":"image_omitted"});
                    }
                }
            }
        }
        if serde_json::to_vec(&text_only)?.len() > 240000 {
            return Err(AiError::Budget("对话文本超过 240 KiB".into()));
        }
        let response = client
            .complete(
                &messages,
                if allow_tools {
                    tools::definitions(config.vision)
                } else {
                    vec![]
                },
                &cancel,
            )
            .await?;
        metrics.rounds = round;
        metrics.usage.push(response.usage);
        let tokens: u64 = metrics
            .usage
            .iter()
            .filter_map(|u| u["total_tokens"].as_u64())
            .fold(0u64, u64::saturating_add);
        if tokens > 100000 {
            return Err(AiError::Budget("累计 Token 超过 100000".into()));
        }
        let message = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AiError::Transport("模型没有返回候选".into()))?
            .message;
        if message["role"] != "assistant" {
            return Err(AiError::Transport("模型消息角色无效".into()));
        }
        messages.push(message.clone());
        if let Some(calls) = message["tool_calls"].as_array().filter(|v| !v.is_empty()) {
            if !allow_tools || calls.len() > usize::from(config.max_tools - metrics.tool_calls) {
                return Err(AiError::Budget("模型工具请求超限".into()));
            }
            let mut supplements = vec![];
            for call in calls {
                if cancel.is_cancelled() {
                    return Err(AiError::Cancelled);
                }
                let id = call["id"]
                    .as_str()
                    .ok_or_else(|| AiError::Invalid("工具缺少调用 ID".into()))?;
                metrics.tool_calls += 1;
                progress(Progress {
                    round,
                    stage: ProgressStage::Inspecting,
                });
                let e = evidence.clone();
                let c = call.clone();
                let vision = config.vision;
                let count = pairs;
                let (result, new_count) = tokio::task::spawn_blocking(move || {
                    let mut n = count;
                    let r = tools::dispatch(&e, &c, vision, &mut n);
                    (r, n)
                })
                .await
                .map_err(|_| AiError::Invalid("证据查询中断".into()))?;
                pairs = new_count;
                let (facts, images, pixels) =
                    result.unwrap_or_else(|e| (json!({"error":e.to_string()}), vec![], 0));
                retained_pixels += pixels;
                supplements.extend(images);
                messages.push(json!({"role":"tool","tool_call_id":id,"content":facts.to_string()}));
            }
            if !supplements.is_empty() {
                messages.push(json!({"role":"user","content":supplements}));
            }
            continue;
        }
        progress(Progress {
            round,
            stage: ProgressStage::Validating,
        });
        let validated = (|| {
            let content = message["content"]
                .as_str()
                .ok_or_else(|| AiError::Invalid("模型未返回 JSON 内容".into()))?;
            let result: InferenceResult = serde_json::from_str(content)
                .map_err(|error| AiError::Invalid(format!("输出契约错误：{error}")))?;
            validation::validate(&result, &evidence, registry)?;
            Ok::<_, AiError>(result)
        })();
        match validated {
            Ok(mut result)=>{result.metrics=metrics;return Ok(result);}
            Err(error) if round<config.max_rounds=>messages.push(json!({"role":"user","content":format!("校验失败：{error}。根据原始证据修正并重新输出完整 JSON；不得省略尾部操作。")})),
            Err(error)=>return Err(error),
        }
    }
    Err(AiError::Budget("达到轮数上限，未取得合格结果".into()))
}
