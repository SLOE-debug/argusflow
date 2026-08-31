//! 单层结构化 Loop Gate 的强连通分量校验。

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use argusflow_core::WorkflowDefinition;

use super::{
    validation_graph::WorkflowGraph,
    validator::{ValidationIssue, ValidationIssueCode, issue},
};
use crate::{NodeFlow, PreparedNode};

/// 校验所有有环 SCC，并拒绝未形成回边的孤立 Loop Gate。
pub(crate) fn validate_structured_loops(
    workflow: &WorkflowDefinition,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    graph: &WorkflowGraph<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    let components = cyclic_components(graph.adjacency());
    let cyclic_nodes = components
        .iter()
        .flat_map(|component| component.iter().cloned())
        .collect::<HashSet<_>>();

    for (node_id, node) in prepared_nodes {
        if matches!(node.flow(), NodeFlow::Loop { .. }) && !cyclic_nodes.contains(node_id) {
            issues.push(issue(
                ValidationIssueCode::InvalidLoop,
                "“重复执行”需要一条返回路径。请把“继续重复”出口连到要重复的步骤，并从这些步骤连回当前节点",
                Some(node_id.clone()),
                None,
            ));
        }
    }

    for component in components {
        validate_component(workflow, prepared_nodes, graph, &component, issues);
    }
}

/// 一个循环 SCC 必须由唯一 Gate 控制全部入口和耗尽出口。
fn validate_component(
    workflow: &WorkflowDefinition,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    graph: &WorkflowGraph<'_>,
    component: &HashSet<String>,
    issues: &mut Vec<ValidationIssue>,
) {
    let gates = component
        .iter()
        .filter(|node_id| {
            prepared_nodes
                .get(*node_id)
                .is_some_and(|node| matches!(node.flow(), NodeFlow::Loop { .. }))
        })
        .collect::<Vec<_>>();
    let [gate_id] = gates.as_slice() else {
        issues.push(issue(
            if gates.is_empty() {
                ValidationIssueCode::CycleDetected
            } else {
                ValidationIssueCode::InvalidLoop
            },
            if gates.is_empty() {
                "流程中出现了没有次数或时间限制的重复路径。请添加一个“重复执行”节点"
            } else {
                "同一段重复路径只能包含一个“重复执行”节点"
            },
            None,
            None,
        ));
        return;
    };
    let gate_id = gate_id.as_str();

    for edge in &workflow.edges {
        let source_inside = component.contains(&edge.source);
        let target_inside = component.contains(&edge.target);
        if !source_inside && target_inside && edge.target != gate_id {
            issues.push(issue(
                ValidationIssueCode::InvalidLoop,
                "请先进入“重复执行”节点，再进入其中的步骤；不要从外部直接跳到中间步骤",
                Some(edge.target.clone()),
                Some(edge.id.clone()),
            ));
        }
    }

    let gate_edges = workflow
        .edges
        .iter()
        .filter(|edge| edge.source == gate_id)
        .collect::<Vec<_>>();
    let iterate = gate_edges.iter().find(|edge| {
        edge.branch
            .as_ref()
            .is_some_and(|branch| branch.as_str() == "iterate")
    });
    let exhausted = gate_edges.iter().find(|edge| {
        edge.branch
            .as_ref()
            .is_some_and(|branch| branch.as_str() == "exhausted")
    });
    if !iterate.is_some_and(|edge| component.contains(&edge.target)) {
        issues.push(issue(
            ValidationIssueCode::InvalidLoop,
            "请把“继续重复”出口连接到需要重复的步骤",
            Some(gate_id.to_owned()),
            iterate.map(|edge| edge.id.clone()),
        ));
    }
    if !exhausted.is_some_and(|edge| !component.contains(&edge.target)) {
        issues.push(issue(
            ValidationIssueCode::InvalidLoop,
            "请把“停止重复”出口连接到重复结束后的下一步",
            Some(gate_id.to_owned()),
            exhausted.map(|edge| edge.id.clone()),
        ));
    }

    let body = component
        .iter()
        .filter(|node_id| node_id.as_str() != gate_id)
        .cloned()
        .collect::<HashSet<_>>();
    let body_adjacency = body
        .iter()
        .map(|node_id| {
            let targets = graph
                .adjacency()
                .get(node_id)
                .into_iter()
                .flatten()
                .filter(|target| body.contains(*target))
                .cloned()
                .collect();
            (node_id.clone(), targets)
        })
        .collect();
    if !cyclic_components(&body_adjacency).is_empty() {
        issues.push(issue(
            ValidationIssueCode::InvalidLoop,
            "重复步骤中还有另一条返回路径。请只保留回到“重复执行”节点的路径",
            Some(gate_id.to_owned()),
            None,
        ));
    }

    for node_id in component {
        if prepared_nodes
            .get(node_id)
            .is_some_and(|node| node.acquires_resources())
        {
            issues.push(issue(
                ValidationIssueCode::InvalidLoop,
                "请在开始重复之前打开应用或浏览器，不要在重复步骤中反复打开",
                Some(node_id.clone()),
                None,
            ));
        }
    }
}

/// 使用 Tarjan 算法返回所有真正含环的强连通分量。
fn cyclic_components(adjacency: &HashMap<String, Vec<String>>) -> Vec<HashSet<String>> {
    let mut state = TarjanState::default();
    let node_ids = adjacency.keys().cloned().collect::<Vec<_>>();
    for node_id in node_ids {
        if !state.indices.contains_key(&node_id) {
            state.visit(&node_id, adjacency);
        }
    }
    state
        .components
        .into_iter()
        .filter(|component| {
            component.len() > 1
                || component.iter().any(|node_id| {
                    adjacency
                        .get(node_id)
                        .is_some_and(|targets| targets.contains(node_id))
                })
        })
        .collect()
}

/// Tarjan DFS 的可变索引、栈与完成分量。
#[derive(Default)]
struct TarjanState {
    /// 下一个 DFS 序号。
    next_index: usize,
    /// 每个已访问节点的首次序号。
    indices: HashMap<String, usize>,
    /// 当前 DFS 子树可达的最小序号。
    low_links: HashMap<String, usize>,
    /// 尚未完成分量的节点栈。
    stack: Vec<String>,
    /// 判断回边目标是否仍在栈中。
    on_stack: HashSet<String>,
    /// 已完成的强连通分量。
    components: Vec<HashSet<String>>,
}

impl TarjanState {
    /// 深度优先访问一个节点，并在根节点处弹出完整分量。
    fn visit(&mut self, node_id: &str, adjacency: &HashMap<String, Vec<String>>) {
        let index = self.next_index;
        self.next_index += 1;
        self.indices.insert(node_id.to_owned(), index);
        self.low_links.insert(node_id.to_owned(), index);
        self.stack.push(node_id.to_owned());
        self.on_stack.insert(node_id.to_owned());

        for target in adjacency.get(node_id).into_iter().flatten() {
            if !self.indices.contains_key(target) {
                self.visit(target, adjacency);
                let target_low = self.low_links[target];
                let current_low = self.low_links[node_id];
                self.low_links
                    .insert(node_id.to_owned(), current_low.min(target_low));
            } else if self.on_stack.contains(target) {
                let target_index = self.indices[target];
                let current_low = self.low_links[node_id];
                self.low_links
                    .insert(node_id.to_owned(), current_low.min(target_index));
            }
        }

        if self.low_links[node_id] != self.indices[node_id] {
            return;
        }
        let mut component = HashSet::new();
        while let Some(member) = self.stack.pop() {
            self.on_stack.remove(&member);
            let done = member == node_id;
            component.insert(member);
            if done {
                break;
            }
        }
        self.components.push(component);
    }
}
