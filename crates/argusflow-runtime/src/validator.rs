use std::collections::{HashMap, HashSet, VecDeque};

use argusflow_core::{ConditionBranch, WorkflowDefinition, WorkflowNodeKind};
use serde::{Deserialize, Serialize};

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
    /// schema 版本不是 2。
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
}

/// 校验 schema v2 条件 DAG、节点参数和分支契约。
pub fn validate_workflow(workflow: &WorkflowDefinition) -> ValidationReport {
    let mut issues = Vec::new();
    if workflow.schema_version != 2 {
        issues.push(issue(
            ValidationIssueCode::UnsupportedSchemaVersion,
            "schema_version 必须为 2",
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
            WorkflowNodeKind::Log { message } => {
                if message.trim().is_empty() {
                    issues.push(issue(
                        ValidationIssueCode::EmptyLogMessage,
                        "Log 节点的消息不能为空",
                        Some(node.id.clone()),
                        None,
                    ));
                }
            }
            WorkflowNodeKind::Delay { milliseconds } => {
                if !(1..=60_000).contains(milliseconds) {
                    issues.push(issue(
                        ValidationIssueCode::InvalidDelay,
                        "Delay 节点必须在 1 到 60000 毫秒之间",
                        Some(node.id.clone()),
                        None,
                    ));
                }
            }
            WorkflowNodeKind::Condition { predicate } => {
                if let Err(error) = predicate.evaluate(&workflow.variables) {
                    issues.push(issue(
                        ValidationIssueCode::InvalidCondition,
                        error.to_string(),
                        Some(node.id.clone()),
                        None,
                    ));
                }
            }
            WorkflowNodeKind::Action { .. } => {}
        }
    }
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

    let mut edge_ids = HashSet::new();
    let mut incoming: HashMap<String, usize> = node_ids.iter().map(|id| (id.clone(), 0)).collect();
    let mut outgoing: HashMap<String, Vec<&argusflow_core::WorkflowEdge>> = node_ids
        .iter()
        .map(|id| (id.clone(), Vec::new()))
        .collect();
    let mut adjacency: HashMap<String, Vec<String>> = node_ids
        .iter()
        .map(|id| (id.clone(), Vec::new()))
        .collect();
    let mut reverse: HashMap<String, Vec<String>> = node_ids
        .iter()
        .map(|id| (id.clone(), Vec::new()))
        .collect();
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
        *incoming.entry(edge.target.clone()).or_default() += 1;
        outgoing.entry(edge.source.clone()).or_default().push(edge);
        adjacency
            .entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
        reverse
            .entry(edge.target.clone())
            .or_default()
            .push(edge.source.clone());
    }

    for node in &workflow.nodes {
        let incoming_count = incoming.get(&node.id).copied().unwrap_or_default();
        let node_edges = outgoing.get(&node.id).map(Vec::as_slice).unwrap_or_default();
        let valid_degree = match &node.kind {
            WorkflowNodeKind::Start => incoming_count == 0 && node_edges.len() == 1,
            WorkflowNodeKind::End => incoming_count >= 1 && node_edges.is_empty(),
            WorkflowNodeKind::Condition { .. } => incoming_count >= 1 && node_edges.len() == 2,
            WorkflowNodeKind::Log { .. }
            | WorkflowNodeKind::Delay { .. }
            | WorkflowNodeKind::Action { .. } => incoming_count >= 1 && node_edges.len() == 1,
        };
        if !valid_degree {
            issues.push(issue(
                ValidationIssueCode::InvalidNodeDegree,
                "节点入度或出度不符合条件 DAG 约束",
                Some(node.id.clone()),
                None,
            ));
        }
        if matches!(&node.kind, WorkflowNodeKind::Condition { .. }) {
            let branches: HashSet<_> = node_edges.iter().filter_map(|edge| edge.branch).collect();
            if branches != HashSet::from([ConditionBranch::True, ConditionBranch::False]) {
                issues.push(issue(
                    ValidationIssueCode::InvalidBranch,
                    "Condition 节点必须具有唯一的 true 和 false 分支",
                    Some(node.id.clone()),
                    None,
                ));
            }
        } else {
            for edge in node_edges.iter().filter(|edge| edge.branch.is_some()) {
                issues.push(issue(
                    ValidationIssueCode::InvalidBranch,
                    "只有 Condition 节点的连线可以包含 branch",
                    Some(node.id.clone()),
                    Some(edge.id.clone()),
                ));
            }
        }
    }

    if has_cycle(&node_ids, &incoming, &adjacency) {
        issues.push(issue(
            ValidationIssueCode::CycleDetected,
            "工作流不能包含环路",
            None,
            None,
        ));
    }
    if start_ids.len() == 1 {
        let reachable = reachable_nodes(&start_ids[0], &adjacency);
        for id in node_ids.difference(&reachable) {
            issues.push(issue(
                ValidationIssueCode::UnreachableNode,
                format!("节点 '{id}' 无法从 Start 到达"),
                Some(id.clone()),
                None,
            ));
        }
    }
    if end_ids.len() == 1 {
        let reaches_end = reachable_nodes(&end_ids[0], &reverse);
        for id in node_ids.difference(&reaches_end) {
            issues.push(issue(
                ValidationIssueCode::NoPathToEnd,
                format!("节点 '{id}' 无法到达 End"),
                Some(id.clone()),
                None,
            ));
        }
    }
    ValidationReport {
        valid: issues.is_empty(),
        issues,
    }
}

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

fn issue(
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
