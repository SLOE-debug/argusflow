use std::collections::{HashMap, HashSet, VecDeque};

use argusflow_core::{ConditionBranch, WorkflowDefinition, WorkflowNodeKind};
use serde::{Deserialize, Serialize};

use crate::{
    validation_nodes::validate_node_parameters, validation_references::validate_data_references,
};

/// 工作流结构校验的汇总结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// 没有任何问题时为 `true`。
    pub valid: bool,
    /// 按校验顺序收集的全部问题。
    pub issues: Vec<ValidationIssue>,
}

/// 一项可定位到节点或连线的工作流校验问题。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// 稳定的机器可读问题码。
    pub code: ValidationIssueCode,
    /// 面向用户展示的中文说明。
    pub message: String,
    /// 相关节点 ID；工作流级问题为空。
    pub node_id: Option<String>,
    /// 相关连线 ID；工作流级问题为空。
    pub edge_id: Option<String>,
}

/// 工作流校验器能够识别的问题类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationIssueCode {
    /// schema 版本不是 4。
    UnsupportedSchemaVersion,
    /// 工作流名称为空。
    EmptyWorkflowName,
    /// variables 根值不是 JSON 对象。
    InvalidVariables,
    /// 节点 ID 为空。
    EmptyNodeId,
    /// 节点 ID 重复。
    DuplicateNodeId,
    /// 连线 ID 为空。
    EmptyEdgeId,
    /// 连线 ID 重复。
    DuplicateEdgeId,
    /// Start 数量不是一个。
    InvalidStartCount,
    /// End 数量不是一个。
    InvalidEndCount,
    /// 连线端点不存在。
    UnknownEdgeEndpoint,
    /// 连线连接回自身。
    SelfLoop,
    /// 节点度数不符合条件 DAG 约束。
    InvalidNodeDegree,
    /// Condition 谓词无法求值。
    InvalidCondition,
    /// Condition 分支标签缺失、重复或用在普通边上。
    InvalidBranch,
    /// 图中存在环路。
    CycleDetected,
    /// 节点不能从 Start 到达。
    UnreachableNode,
    /// 节点没有到 End 的路径。
    NoPathToEnd,
    /// Log 消息为空。
    EmptyLogMessage,
    /// Delay 时长越界。
    InvalidDelay,
    /// UI 节点携带的 AQL 无法解析或通过语义检查。
    InvalidAqlQuery,
    /// 应用节点缺少有效 EXE、窗口标题或策略配置。
    InvalidApplicationSpec,
    /// CommandOperation 的 runner 与字段组合或资源上限无效。
    InvalidCommand,
    /// WorkflowPermissions 没有授权 Command 节点所需能力。
    CommandPermissionDenied,
    /// ValueExpr 引用了无效输入、变量或节点输出。
    InvalidValueReference,
    /// TargetScope 引用了无效应用资源输出。
    InvalidResourceReference,
    /// 值或资源生产节点没有支配消费节点。
    ReferenceNotDominating,
}

/// 校验 schema v4 条件 DAG、节点参数、数据流和资源支配关系。
pub fn validate_workflow(workflow: &WorkflowDefinition) -> ValidationReport {
    let mut issues = Vec::new();
    validate_workflow_metadata(workflow, &mut issues);

    let mut node_ids = HashSet::new();
    let mut start_ids = Vec::new();
    let mut end_ids = Vec::new();
    for node in &workflow.nodes {
        if node.id.trim().is_empty() {
            issues.push(issue(
                ValidationIssueCode::EmptyNodeId,
                "节点 ID 不能为空",
                Some(node.id.clone()),
                None,
            ));
        }
        if !node_ids.insert(node.id.clone()) {
            issues.push(issue(
                ValidationIssueCode::DuplicateNodeId,
                format!("节点 ID '{}' 重复", node.id),
                Some(node.id.clone()),
                None,
            ));
        }
        match &node.kind {
            WorkflowNodeKind::Start => start_ids.push(node.id.clone()),
            WorkflowNodeKind::End => end_ids.push(node.id.clone()),
            _ => {}
        }
        validate_node_parameters(node, workflow, &mut issues);
    }
    validate_terminal_counts(&start_ids, &end_ids, &mut issues);

    let graph = build_graph(workflow, &node_ids, &mut issues);
    validate_node_degrees(workflow, &graph, &mut issues);
    validate_graph_shape(&node_ids, &start_ids, &end_ids, &graph, &mut issues);
    if start_ids.len() == 1 {
        validate_data_references(workflow, &start_ids[0], &graph.predecessors, &mut issues);
    }

    ValidationReport {
        valid: issues.is_empty(),
        issues,
    }
}

/// 校验工作流级契约。
fn validate_workflow_metadata(workflow: &WorkflowDefinition, issues: &mut Vec<ValidationIssue>) {
    if workflow.schema_version != 4 {
        issues.push(issue(
            ValidationIssueCode::UnsupportedSchemaVersion,
            "schema_version 必须为 4",
            None,
            None,
        ));
    }
    if workflow.name.trim().is_empty() {
        issues.push(issue(
            ValidationIssueCode::EmptyWorkflowName,
            "工作流名称不能为空",
            None,
            None,
        ));
    }
    if !workflow.variables.is_object() {
        issues.push(issue(
            ValidationIssueCode::InvalidVariables,
            "工作流变量根值必须是 JSON 对象",
            None,
            None,
        ));
    }
}

/// 校验唯一 Start 与 End 数量。
fn validate_terminal_counts(
    start_ids: &[String],
    end_ids: &[String],
    issues: &mut Vec<ValidationIssue>,
) {
    if start_ids.len() != 1 {
        issues.push(issue(
            ValidationIssueCode::InvalidStartCount,
            "工作流必须且只能包含一个 Start 节点",
            None,
            None,
        ));
    }
    if end_ids.len() != 1 {
        issues.push(issue(
            ValidationIssueCode::InvalidEndCount,
            "工作流必须且只能包含一个 End 节点",
            None,
            None,
        ));
    }
}

/// Validator 内部使用的有向图索引。
struct WorkflowGraph<'workflow> {
    /// 每个节点的入度。
    incoming: HashMap<String, usize>,
    /// 每个节点按文档顺序排列的出边。
    outgoing: HashMap<String, Vec<&'workflow argusflow_core::WorkflowEdge>>,
    /// 从节点到后继节点 ID 的邻接表。
    adjacency: HashMap<String, Vec<String>>,
    /// 从节点到前驱节点 ID 的反向邻接表。
    predecessors: HashMap<String, Vec<String>>,
}

/// 校验连线身份和端点并建立图索引。
fn build_graph<'workflow>(
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

/// 校验各类节点的入度、出度和条件分支标签。
fn validate_node_degrees(
    workflow: &WorkflowDefinition,
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
        let valid_degree = match &node.kind {
            WorkflowNodeKind::Start => incoming_count == 0 && node_edges.len() == 1,
            WorkflowNodeKind::End => incoming_count >= 1 && node_edges.is_empty(),
            WorkflowNodeKind::Condition { .. } => incoming_count >= 1 && node_edges.len() == 2,
            WorkflowNodeKind::Log { .. }
            | WorkflowNodeKind::Debug { .. }
            | WorkflowNodeKind::Delay { .. }
            | WorkflowNodeKind::Application { .. }
            | WorkflowNodeKind::Ui { .. }
            | WorkflowNodeKind::Command { .. } => incoming_count >= 1 && node_edges.len() == 1,
        };
        if !valid_degree {
            issues.push(issue(
                ValidationIssueCode::InvalidNodeDegree,
                "节点入度或出度不符合条件 DAG 约束",
                Some(node.id.clone()),
                None,
            ));
        }
        validate_branches(node, node_edges, issues);
    }
}

/// 条件节点必须有唯一 true/false，普通节点不能携带 branch。
fn validate_branches(
    node: &argusflow_core::WorkflowNode,
    edges: &[&argusflow_core::WorkflowEdge],
    issues: &mut Vec<ValidationIssue>,
) {
    if matches!(&node.kind, WorkflowNodeKind::Condition { .. }) {
        let branches: HashSet<_> = edges.iter().filter_map(|edge| edge.branch).collect();
        if branches != HashSet::from([ConditionBranch::True, ConditionBranch::False]) {
            issues.push(issue(
                ValidationIssueCode::InvalidBranch,
                "Condition 节点必须具有唯一的 true 和 false 分支",
                Some(node.id.clone()),
                None,
            ));
        }
    } else {
        for edge in edges.iter().filter(|edge| edge.branch.is_some()) {
            issues.push(issue(
                ValidationIssueCode::InvalidBranch,
                "只有 Condition 节点的连线可以包含 branch",
                Some(node.id.clone()),
                Some(edge.id.clone()),
            ));
        }
    }
}

/// 校验 DAG、Start 可达性和到 End 的可达性。
fn validate_graph_shape(
    node_ids: &HashSet<String>,
    start_ids: &[String],
    end_ids: &[String],
    graph: &WorkflowGraph<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    if has_cycle(node_ids, &graph.incoming, &graph.adjacency) {
        issues.push(issue(
            ValidationIssueCode::CycleDetected,
            "工作流不能包含环路",
            None,
            None,
        ));
    }
    if let [start_id] = start_ids {
        let reachable = reachable_nodes(start_id, &graph.adjacency);
        for id in node_ids.difference(&reachable) {
            issues.push(issue(
                ValidationIssueCode::UnreachableNode,
                format!("节点 '{id}' 无法从 Start 到达"),
                Some(id.clone()),
                None,
            ));
        }
    }
    if let [end_id] = end_ids {
        let reaches_end = reachable_nodes(end_id, &graph.predecessors);
        for id in node_ids.difference(&reaches_end) {
            issues.push(issue(
                ValidationIssueCode::NoPathToEnd,
                format!("节点 '{id}' 无法到达 End"),
                Some(id.clone()),
                None,
            ));
        }
    }
}

/// 使用 Kahn 算法判断已知节点子图是否含环。
fn has_cycle(
    node_ids: &HashSet<String>,
    incoming: &HashMap<String, usize>,
    adjacency: &HashMap<String, Vec<String>>,
) -> bool {
    let mut counts = incoming.clone();
    let mut queue: VecDeque<_> = node_ids
        .iter()
        .filter(|id| counts.get(*id).copied().unwrap_or_default() == 0)
        .cloned()
        .collect();
    let mut processed = 0;
    while let Some(id) = queue.pop_front() {
        processed += 1;
        for target in adjacency.get(&id).into_iter().flatten() {
            let count = counts.entry(target.clone()).or_default();
            *count = count.saturating_sub(1);
            if *count == 0 {
                queue.push_back(target.clone());
            }
        }
    }
    processed != node_ids.len()
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

/// 创建一项稳定且可定位的校验问题。
pub(crate) fn issue(
    code: ValidationIssueCode,
    message: impl Into<String>,
    node_id: Option<String>,
    edge_id: Option<String>,
) -> ValidationIssue {
    ValidationIssue {
        code,
        message: message.into(),
        node_id,
        edge_id,
    }
}
