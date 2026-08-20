use std::collections::{HashMap, HashSet, VecDeque};

use argusflow_core::{WorkflowDefinition, WorkflowNodeKind};
use serde::{Deserialize, Serialize};

/// 工作流结构校验的汇总结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// 没有任何问题时为 `true`。
    pub valid: bool,
    /// 按校验顺序收集的全部问题，而不是遇到首个错误就停止。
    pub issues: Vec<ValidationIssue>,
}

/// 一项可定位到节点或连线的工作流校验问题。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// 稳定的机器可读问题代码。
    pub code: ValidationIssueCode,
    /// 面向用户的中文说明。
    pub message: String,
    /// 相关节点 ID；工作流级问题时为空。
    pub node_id: Option<String>,
    /// 相关连线 ID；工作流级问题时为空。
    pub edge_id: Option<String>,
}

/// 工作流校验器能够识别的问题类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationIssueCode {
    /// 工作流契约版本不是运行时支持的版本。
    UnsupportedSchemaVersion,
    /// 工作流名称为空或只包含空白。
    EmptyWorkflowName,
    /// 节点 ID 为空或只包含空白。
    EmptyNodeId,
    /// 节点 ID 在同一工作流中重复。
    DuplicateNodeId,
    /// 连线 ID 为空或只包含空白。
    EmptyEdgeId,
    /// 连线 ID 在同一工作流中重复。
    DuplicateEdgeId,
    /// Start 节点数量不是一个。
    InvalidStartCount,
    /// End 节点数量不是一个。
    InvalidEndCount,
    /// 连线端点引用了不存在的节点。
    UnknownEdgeEndpoint,
    /// 连线把节点连接回自身。
    SelfLoop,
    /// 节点入度或出度不符合线性链约束。
    InvalidNodeDegree,
    /// 工作流拓扑中存在环路。
    CycleDetected,
    /// 节点无法从唯一 Start 节点到达。
    UnreachableNode,
    /// Log 节点消息为空或只包含空白。
    EmptyLogMessage,
    /// Delay 节点时长超出 1 到 60000 毫秒的允许范围。
    InvalidDelay,
}

/// 校验工作流契约及首版线性执行约束。
///
/// 函数会完整收集结构问题，包括 ID、端点、节点度数、环路和可达性，调用方可一次性
/// 展示所有修复项。校验不会修改输入工作流。
pub fn validate_workflow(workflow: &WorkflowDefinition) -> ValidationReport {
    let mut issues = Vec::new();

    if workflow.schema_version != 1 {
        issues.push(issue(
            ValidationIssueCode::UnsupportedSchemaVersion,
            "schema_version 必须为 1",
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

    let mut node_ids = HashSet::new();
    let mut start_ids = Vec::new();
    let mut end_ids = Vec::new();

    // 先建立节点集合和 Start/End 候选，后续连线与可达性检查依赖这些索引。
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
            WorkflowNodeKind::Log { message } if message.trim().is_empty() => issues.push(issue(
                ValidationIssueCode::EmptyLogMessage,
                "Log 节点的消息不能为空",
                Some(node.id.clone()),
                None,
            )),
            WorkflowNodeKind::Delay { milliseconds } if !(1..=60_000).contains(milliseconds) => {
                issues.push(issue(
                    ValidationIssueCode::InvalidDelay,
                    "Delay 节点必须在 1 到 60000 毫秒之间",
                    Some(node.id.clone()),
                    None,
                ));
            }
            _ => {}
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
    let mut incoming: HashMap<String, usize> = HashMap::new();
    let mut outgoing: HashMap<String, usize> = HashMap::new();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

    // 初始化所有节点的度数，即使节点没有连线也能报告单独的度数错误。
    for node_id in &node_ids {
        incoming.insert(node_id.clone(), 0);
        outgoing.insert(node_id.clone(), 0);
        adjacency.insert(node_id.clone(), Vec::new());
    }

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

        *outgoing.entry(edge.source.clone()).or_default() += 1;
        *incoming.entry(edge.target.clone()).or_default() += 1;
        adjacency
            .entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
    }

    for node in &workflow.nodes {
        let incoming_count = incoming.get(&node.id).copied().unwrap_or_default();
        let outgoing_count = outgoing.get(&node.id).copied().unwrap_or_default();
        let valid_degree = match &node.kind {
            WorkflowNodeKind::Start => incoming_count == 0 && outgoing_count == 1,
            WorkflowNodeKind::End => incoming_count == 1 && outgoing_count == 0,
            _ => incoming_count == 1 && outgoing_count == 1,
        };

        if !valid_degree {
            issues.push(issue(
                ValidationIssueCode::InvalidNodeDegree,
                "首版工作流只支持单入单出的线性执行链",
                Some(node.id.clone()),
                None,
            ));
        }
    }

    // 拓扑处理既能识别环路，也不会被后续可达性检查误判为执行成功。
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
        for node_id in node_ids.difference(&reachable) {
            issues.push(issue(
                ValidationIssueCode::UnreachableNode,
                format!("节点 '{node_id}' 无法从 Start 到达"),
                Some(node_id.clone()),
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
    // Kahn 算法逐步移除入度为零的节点；若最终仍有节点未处理，则存在环路。
    let mut remaining_incoming = incoming.clone();
    let mut queue: VecDeque<String> = node_ids
        .iter()
        .filter(|id| remaining_incoming.get(*id).copied().unwrap_or_default() == 0)
        .cloned()
        .collect();
    let mut processed = 0;

    while let Some(node_id) = queue.pop_front() {
        processed += 1;
        if let Some(targets) = adjacency.get(&node_id) {
            for target in targets {
                if let Some(count) = remaining_incoming.get_mut(target) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        queue.push_back(target.clone());
                    }
                }
            }
        }
    }

    processed != node_ids.len()
}

fn reachable_nodes(start_id: &str, adjacency: &HashMap<String, Vec<String>>) -> HashSet<String> {
    // 使用广度优先遍历收集从唯一 Start 可到达的节点，避免孤立节点漏过校验。
    let mut reachable = HashSet::new();
    let mut queue = VecDeque::from([start_id.to_owned()]);

    while let Some(node_id) = queue.pop_front() {
        if !reachable.insert(node_id.clone()) {
            continue;
        }
        if let Some(targets) = adjacency.get(&node_id) {
            queue.extend(targets.iter().cloned());
        }
    }

    reachable
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
