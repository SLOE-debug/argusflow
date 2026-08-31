use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use argusflow_core::{ControlPortId, WorkflowDefinition, WorkflowEdge, WorkflowNode};

use super::validator::{ValidationIssue, ValidationIssueCode, issue};
use crate::node_registry::{NodeFlow, PreparedNode};

/// Validator 内部使用的有向图索引。
pub(crate) struct WorkflowGraph<'workflow> {
    /// 每个节点的入度。
    incoming: HashMap<String, usize>,
    /// 每个节点按文档顺序排列的出边。
    outgoing: HashMap<String, Vec<&'workflow WorkflowEdge>>,
    /// 从节点到后继节点 ID 的邻接表。
    pub(super) adjacency: HashMap<String, Vec<String>>,
    /// 从节点到前驱节点 ID 的反向邻接表。
    pub(crate) predecessors: HashMap<String, Vec<String>>,
}

impl WorkflowGraph<'_> {
    /// 返回结构化循环校验使用的只读邻接表。
    pub(super) const fn adjacency(&self) -> &HashMap<String, Vec<String>> {
        &self.adjacency
    }
}

/// 校验连线身份和端点并建立图索引。
pub(crate) fn build_graph<'workflow>(
    workflow: &'workflow WorkflowDefinition,
    node_ids: &HashSet<String>,
    issues: &mut Vec<ValidationIssue>,
) -> WorkflowGraph<'workflow> {
    let mut edge_ids = HashSet::new();
    let mut graph = WorkflowGraph {
        incoming: node_ids.iter().map(|id| (id.clone(), 0)).collect(),
        outgoing: node_ids.iter().map(|id| (id.clone(), Vec::new())).collect(),
        adjacency: node_ids.iter().map(|id| (id.clone(), Vec::new())).collect(),
        predecessors: node_ids.iter().map(|id| (id.clone(), Vec::new())).collect(),
    };
    for edge in &workflow.edges {
        if edge.id.trim().is_empty() {
            issues.push(issue(
                ValidationIssueCode::EmptyEdgeId,
                "连线 ID 不能为空",
                None,
                Some(edge.id.clone()),
            ));
        }
        if !edge_ids.insert(edge.id.clone()) {
            issues.push(issue(
                ValidationIssueCode::DuplicateEdgeId,
                format!("连线 ID '{}' 重复", edge.id),
                None,
                Some(edge.id.clone()),
            ));
        }
        if !node_ids.contains(&edge.source) || !node_ids.contains(&edge.target) {
            issues.push(issue(
                ValidationIssueCode::UnknownEdgeEndpoint,
                "连线引用了不存在的节点",
                None,
                Some(edge.id.clone()),
            ));
            continue;
        }
        if edge.source == edge.target {
            issues.push(issue(
                ValidationIssueCode::SelfLoop,
                "节点不能连接到自身",
                Some(edge.source.clone()),
                Some(edge.id.clone()),
            ));
        }
        *graph.incoming.entry(edge.target.clone()).or_default() += 1;
        graph
            .outgoing
            .entry(edge.source.clone())
            .or_default()
            .push(edge);
        graph
            .adjacency
            .entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
        graph
            .predecessors
            .entry(edge.target.clone())
            .or_default()
            .push(edge.source.clone());
    }
    graph
}

/// 校验各类节点的入度、出度和分支标签。
pub(crate) fn validate_node_degrees(
    workflow: &WorkflowDefinition,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    graph: &WorkflowGraph<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    for node in &workflow.nodes {
        let incoming_count = graph.incoming.get(&node.id).copied().unwrap_or_default();
        let node_edges = graph
            .outgoing
            .get(&node.id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let Some(prepared) = prepared_nodes.get(&node.id) else {
            continue;
        };
        let flow = prepared.flow();
        let valid_degree = match &flow {
            NodeFlow::Start => incoming_count == 0 && node_edges.len() == 1,
            NodeFlow::End => incoming_count >= 1 && node_edges.is_empty(),
            NodeFlow::Branch { ports } | NodeFlow::Loop { ports } => {
                incoming_count >= 1 && node_edges.len() == ports.len()
            }
            NodeFlow::Linear => incoming_count >= 1 && node_edges.len() == 1,
        };
        if !valid_degree {
            issues.push(issue(
                ValidationIssueCode::InvalidNodeDegree,
                "这个节点的连接数量不正确。请检查进入和离开节点的连线",
                Some(node.id.clone()),
                None,
            ));
        }
        validate_branches(node, &flow, node_edges, issues);
    }
}

/// 分支节点必须覆盖注册端口，普通节点不能携带 branch。
fn validate_branches(
    node: &WorkflowNode,
    flow: &NodeFlow,
    edges: &[&WorkflowEdge],
    issues: &mut Vec<ValidationIssue>,
) {
    if let NodeFlow::Branch { ports } | NodeFlow::Loop { ports } = flow {
        let branches: HashSet<_> = edges
            .iter()
            .filter_map(|edge| edge.branch.as_ref().cloned())
            .collect();
        let expected = ports.iter().cloned().collect::<HashSet<ControlPortId>>();
        if branches != expected || branches.len() != edges.len() {
            issues.push(issue(
                ValidationIssueCode::InvalidBranch,
                "请为这个节点的每一种结果各连接一个下一步",
                Some(node.id.clone()),
                None,
            ));
        }
    } else {
        for edge in edges.iter().filter(|edge| edge.branch.is_some()) {
            issues.push(issue(
                ValidationIssueCode::InvalidBranch,
                "这条连线不需要选择结果，请清除它的分支设置",
                Some(node.id.clone()),
                Some(edge.id.clone()),
            ));
        }
    }
}

/// 校验 DAG、Start 可达性和到 End 的可达性。
pub(crate) fn validate_graph_shape(
    node_ids: &HashSet<String>,
    start_ids: &[String],
    end_ids: &[String],
    graph: &WorkflowGraph<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    if let [start_id] = start_ids {
        let reachable = reachable_nodes(start_id, &graph.adjacency);
        for id in node_ids.difference(&reachable) {
            issues.push(issue(
                ValidationIssueCode::UnreachableNode,
                format!("节点“{id}”不会被运行。请把它连接到开始节点后的流程中"),
                Some(id.clone()),
                None,
            ));
        }
    }
    if !end_ids.is_empty() {
        let reaches_end = reachable_from_many(end_ids, &graph.predecessors);
        for id in node_ids.difference(&reaches_end) {
            issues.push(issue(
                ValidationIssueCode::NoPathToEnd,
                format!("节点“{id}”之后没有可到达的结束节点。请补全后续连线"),
                Some(id.clone()),
                None,
            ));
        }
    }
}

/// 沿指定邻接关系返回包括起点在内的全部可达节点。
fn reachable_nodes(start: &str, adjacency: &HashMap<String, Vec<String>>) -> HashSet<String> {
    let mut reached = HashSet::new();
    let mut queue = VecDeque::from([start.to_owned()]);
    while let Some(id) = queue.pop_front() {
        if !reached.insert(id.clone()) {
            continue;
        }
        queue.extend(adjacency.get(&id).into_iter().flatten().cloned());
    }
    reached
}

/// 从多个终点沿反向图一次性计算可达集合。
fn reachable_from_many(
    starts: &[String],
    adjacency: &HashMap<String, Vec<String>>,
) -> HashSet<String> {
    let mut reached = HashSet::new();
    let mut queue = VecDeque::from(starts.to_vec());
    while let Some(id) = queue.pop_front() {
        if !reached.insert(id.clone()) {
            continue;
        }
        queue.extend(adjacency.get(&id).into_iter().flatten().cloned());
    }
    reached
}
