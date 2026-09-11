//! 无递归的依赖图按后序编译；每份定义保留独立词法根。
use super::{PreparedWorkflow, prepare::compile};
use crate::NodeRegistry;
use argusflow_workflow::{Action, Diagnostic, DiagnosticCode, WorkflowBundle, WorkflowId};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// 冻结依赖后编译所有可达文档，拒绝缺失目标、递归和超额依赖。
pub fn prepare_bundle(
    bundle: WorkflowBundle,
    registry: &NodeRegistry,
) -> Result<Arc<PreparedWorkflow>, Vec<Diagnostic>> {
    let mut compiled = BTreeMap::new();
    visit(
        &bundle.root,
        &bundle,
        registry,
        &mut BTreeSet::new(),
        &mut compiled,
    )
    .map_err(|error| vec![error])
}
fn visit(
    id: &WorkflowId,
    bundle: &WorkflowBundle,
    registry: &NodeRegistry,
    active: &mut BTreeSet<WorkflowId>,
    compiled: &mut BTreeMap<WorkflowId, Arc<PreparedWorkflow>>,
) -> Result<Arc<PreparedWorkflow>, Diagnostic> {
    let error = |message: String| Diagnostic {
        workflow: Some(id.clone()),
        code: DiagnosticCode::Reference,
        message,
        scope: None,
        node: None,
    };
    if let Some(plan) = compiled.get(id) {
        return Ok(plan.clone());
    }
    if id.0.is_empty() || id.0.len() > 256 || bundle.workflows.len() > 256 {
        return Err(error("工作流身份或依赖数量超限".into()));
    }
    if active.len() >= 64 || !active.insert(id.clone()) {
        return Err(error(format!("工作流递归或调用超过 64 层：{}", id.0)));
    }
    let workflow = bundle
        .workflows
        .get(id)
        .ok_or_else(|| error(format!("找不到工作流：{}", id.0)))?;
    for scope in &workflow.scopes {
        for node in &scope.nodes {
            if let Action::CallWorkflow {
                workflow: target, ..
            } = &node.action
            {
                visit(target, bundle, registry, active, compiled).map_err(|mut diagnostic| {
                    diagnostic.message = format!("{} → {}：{}", id.0, target.0, diagnostic.message);
                    if diagnostic.node.is_none() {
                        diagnostic.workflow = Some(id.clone());
                        diagnostic.node = Some(node.id.clone());
                        diagnostic.scope = Some(scope.id.clone());
                    }
                    diagnostic
                })?;
            }
        }
    }
    let mut plan = compile(workflow.clone(), registry, compiled).map_err(|mut diagnostic| {
        diagnostic.workflow = Some(id.clone());
        diagnostic.message = format!("{}：{}", workflow.name, diagnostic.message);
        diagnostic
    })?;
    plan.identity = Some(id.clone());
    let plan = Arc::new(plan);
    active.remove(id);
    compiled.insert(id.clone(), plan.clone());
    Ok(plan)
}
