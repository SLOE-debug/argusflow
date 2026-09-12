//! 显式作用域图的结构检查与邻接索引，不决定分支执行策略。
use crate::{Action, Diagnostic, DiagnosticCode, Node, Scope};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// 作用域边界不产生伪执行节点。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EdgeEndpoint {
    /// 唯一入口边界。
    Start,
    /// 正常完成边界。
    End,
    /// 当前作用域中的动作。
    Node {
        /// 节点身份。
        node: String,
    },
}
impl EdgeEndpoint {
    /// 创建节点端点。
    pub fn node(id: impl Into<String>) -> Self {
        Self::Node { node: id.into() }
    }
    /// 诊断定位使用动作 ID；起止边界定位到作用域。
    pub fn node_id(&self) -> Option<&str> {
        match self {
            Self::Node { node } => Some(node),
            _ => None,
        }
    }
}

/// 稳定边身份与方向，端口边位由编辑布局独立保存。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEdge {
    /// 文档内唯一身份，改接时保持。
    pub id: String,
    /// 只能是开始或普通动作。
    pub source: EdgeEndpoint,
    /// 只能是普通动作或结束。
    pub target: EdgeEndpoint,
}

impl Scope {
    /// 从有序动作构造显式线性图；终止动作不连接正常结束。
    pub fn linear(id: impl Into<String>, nodes: Vec<Node>) -> Self {
        let id = id.into();
        let mut edges = Vec::new();
        let mut source = EdgeEndpoint::Start;
        for node in &nodes {
            edges.push(WorkflowEdge {
                id: format!("{id}:edge:{}", edges.len()),
                source,
                target: EdgeEndpoint::node(&node.id),
            });
            source = EdgeEndpoint::node(&node.id);
        }
        if nodes
            .last()
            .is_none_or(|node| !is_terminal_action(&node.action))
        {
            edges.push(WorkflowEdge {
                id: format!("{id}:edge:{}", edges.len()),
                source,
                target: EdgeEndpoint::End,
            });
        }
        Self {
            id,
            edges,
            nodes,
            outputs: BTreeMap::new(),
        }
    }
}

/// 这些动作不会正常继续到其他节点。
pub fn is_terminal_action(action: &Action) -> bool {
    matches!(
        action,
        Action::Return { .. } | Action::Fail { .. } | Action::Break | Action::Continue
    )
}

/// 已验证的只读邻接索引；草稿可以断开或包含多个出口。
pub struct ScopeGraph<'a> {
    scope: &'a Scope,
    outgoing: BTreeMap<EdgeEndpoint, Vec<&'a WorkflowEdge>>,
}
impl<'a> ScopeGraph<'a> {
    /// 检查引用、方向、重复边及回环，不要求连通或单出口。
    pub fn new(scope: &'a Scope) -> Result<Self, Diagnostic> {
        let mut graph = Self {
            scope,
            outgoing: BTreeMap::new(),
        };
        let nodes: BTreeMap<_, _> = scope
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        let mut identities = BTreeSet::new();
        let mut pairs = BTreeSet::new();
        let mut degrees = BTreeMap::from([(EdgeEndpoint::Start, 0usize), (EdgeEndpoint::End, 0)]);
        for id in nodes.keys() {
            degrees.insert(EdgeEndpoint::node(*id), 0);
        }
        if scope.edges.len() > 100_000 {
            return Err(graph.error(None, "连线数量超过预算"));
        }
        for edge in &scope.edges {
            let at = edge.source.node_id();
            if edge.id.trim().is_empty() || edge.id.len() > 256 || !identities.insert(&edge.id) {
                return Err(graph.error(at, "连线身份为空、过长或重复"));
            }
            if edge.source == EdgeEndpoint::End || edge.target == EdgeEndpoint::Start {
                return Err(graph.error(at, "开始只能出线，结束只能接收入线"));
            }
            for endpoint in [&edge.source, &edge.target] {
                if endpoint.node_id().is_some_and(|id| !nodes.contains_key(id)) {
                    return Err(graph.error(endpoint.node_id(), "连线端点必须属于当前作用域"));
                }
            }
            if at.is_some_and(|id| is_terminal_action(&nodes[id].action)) {
                return Err(graph.error(at, "终止节点不能继续连线"));
            }
            if edge.source == edge.target || !pairs.insert((&edge.source, &edge.target)) {
                return Err(graph.error(at, "不允许自环或重复连接"));
            }
            *degrees.entry(edge.target.clone()).or_default() += 1;
            graph
                .outgoing
                .entry(edge.source.clone())
                .or_default()
                .push(edge);
        }
        // Kahn 遍历不依赖递归深度，也检查未连接入口的孤立子图。
        let mut pending: VecDeque<_> = degrees
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(id, _)| id.clone())
            .collect();
        let mut visited = 0;
        while let Some(source) = pending.pop_front() {
            visited += 1;
            for edge in graph.outgoing(&source) {
                if let Some(count) = degrees.get_mut(&edge.target) {
                    *count -= 1;
                    if *count == 0 {
                        pending.push_back(edge.target.clone());
                    }
                }
            }
        }
        if visited != degrees.len() {
            return Err(graph.error(None, "不能创建回环，请使用循环节点"));
        }
        Ok(graph)
    }
    /// 读取所有出口，不默默选择第一条。
    pub fn outgoing(&self, source: &EdgeEndpoint) -> &[&'a WorkflowEdge] {
        self.outgoing.get(source).map(Vec::as_slice).unwrap_or(&[])
    }
    /// 条件连线尚未支持执行；运行前对整个作用域检查。
    pub fn require_single_outputs(&self) -> Result<(), Diagnostic> {
        for (source, edges) in &self.outgoing {
            if edges.len() > 1 {
                return Err(self.error(
                    source.node_id(),
                    "此节点有多个出口，需配置分支条件后才能运行",
                ));
            }
        }
        Ok(())
    }
    /// 读取唯一后继；缺失和歧义都返回可定位错误。
    pub fn successor(&self, source: &EdgeEndpoint) -> Result<&'a EdgeEndpoint, Diagnostic> {
        match self.outgoing(source) {
            [edge] => Ok(&edge.target),
            [] => Err(self.error(source.node_id(), "流程尚未连接完整，请连接到结束")),
            _ => Err(self.error(
                source.node_id(),
                "此节点有多个出口，需配置分支条件后才能运行",
            )),
        }
    }
    fn error(&self, node: Option<&str>, message: &str) -> Diagnostic {
        Diagnostic {
            workflow: None,
            code: DiagnosticCode::Structure,
            message: message.into(),
            scope: Some(self.scope.id.clone()),
            node: node.map(str::to_owned),
        }
    }
}
