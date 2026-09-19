//! 模型输出必须通过真实任务编译和完整来源覆盖，不以 JSON 合法替代可用性。
use super::model::*;
use crate::{AiError, Evidence, Result};
use argusflow_runtime::{NodeRegistry, prepare};
use argusflow_workflow::Action;
use std::collections::BTreeSet;
pub(crate) fn validate(
    result: &InferenceResult,
    evidence: &Evidence,
    registry: &NodeRegistry,
) -> Result<()> {
    let reject = |s: &str| AiError::Invalid(s.into());
    if result.analysis.replay_ready {
        return Err(reject("未经回放不得宣称 replay_ready"));
    }
    let known: BTreeSet<_> = evidence.records.keys().map(ToString::to_string).collect();
    let mut covered = BTreeSet::new();
    for ids in result
        .analysis
        .node_evidence
        .iter()
        .map(|n| &n.evidence_ids)
        .chain(result.analysis.unresolved.iter().map(|n| &n.evidence_ids))
    {
        if ids.is_empty() || ids.iter().any(|id| !known.contains(id)) {
            return Err(reject("存在空或无效的证据引用"));
        }
        covered.extend(ids.iter().cloned());
    }
    let missed: Vec<_> = evidence
        .required_ids()
        .into_iter()
        .filter(|id| !covered.contains(id))
        .collect();
    if !missed.is_empty() {
        return Err(AiError::Invalid(format!(
            "以下操作未解释：{}",
            missed.join(",")
        )));
    }
    let Some(workflow) = &result.workflow else {
        if result.analysis.unresolved.is_empty() {
            return Err(reject("无工作流时必须说明未解决的问题"));
        }
        return Ok(());
    };
    if !result.analysis.unresolved.is_empty() {
        return Err(reject("存在未解决操作时不得输出完整工作流"));
    }
    if workflow.scopes.len() != 1 || !workflow.subflows.is_empty() {
        return Err(reject("当前生成契约仅支持单作用域线性流程"));
    }
    let nodes = &workflow.scopes[0].nodes;
    if nodes.is_empty() {
        return Err(reject("不能用空流程替代录制"));
    }
    let node_ids: BTreeSet<_> = nodes.iter().map(|n| n.id.as_str()).collect();
    let refs: BTreeSet<_> = result
        .analysis
        .node_evidence
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();
    if node_ids != refs || refs.len() != result.analysis.node_evidence.len() {
        return Err(reject("节点与证据映射必须一一对应"));
    }
    for node in nodes {
        match &node.action {
            Action::Task { task } => {
                if task.retry.is_some() || node.timeout_ms.is_none() {
                    return Err(reject("任务必须设置有限超时且禁止自动重试"));
                }
            }
            Action::Wait { .. } | Action::Release { .. } => {}
            _ => return Err(reject("本轮输出了生成契约之外的动作")),
        }
    }
    for name in workflow.inputs.keys() {
        if !result
            .analysis
            .required_bindings
            .iter()
            .any(|b| b.name == *name && matches!(b.kind, BindingKind::Input))
        {
            return Err(reject("缺少输入绑定说明"));
        }
    }
    for name in workflow.resources.keys() {
        if !result
            .analysis
            .required_bindings
            .iter()
            .any(|b| b.name == *name && matches!(b.kind, BindingKind::Resource))
        {
            return Err(reject("缺少资源绑定说明"));
        }
    }
    prepare(workflow.clone(), registry)
        .map_err(|e| AiError::Invalid(format!("工作流编译失败：{e:?}")))?;
    Ok(())
}
