//! 后端构造的快照树不暴露可变内部节点。
use crate::{Attribute, Boundary, Role, Value};
use argusflow_core::{Failure, FailureKind, Operation};
use std::collections::{BTreeMap, BTreeSet};

/// 真正的元素与显式边界根有不同类型。
#[derive(Debug, Clone, Copy)]
pub enum NodeKind {
    /// 可以交付给调用方的元素。
    Element(Role),
    /// 只能用作查询范围的文档根。
    Boundary(Boundary),
}
/// 带后端私有身份的只读快照节点。
#[derive(Debug, Clone)]
pub struct Node<T> {
    parent: Option<usize>,
    kind: NodeKind,
    attributes: BTreeMap<Attribute, Value>,
    css: BTreeSet<String>,
    target: Option<T>,
}
impl<T> Node<T> {
    /// 创建元素；父节点必须先于当前节点插入 QueryTree。
    pub fn element(
        parent: Option<usize>,
        role: Role,
        attributes: BTreeMap<Attribute, Value>,
        target: T,
    ) -> Self {
        Self {
            parent,
            kind: NodeKind::Element(role),
            attributes,
            css: BTreeSet::new(),
            target: Some(target),
        }
    }
    /// 创建 iframe 或 Shadow 的范围根。
    pub fn boundary(parent: usize, boundary: Boundary) -> Self {
        Self {
            parent: Some(parent),
            kind: NodeKind::Boundary(boundary),
            attributes: BTreeMap::new(),
            css: BTreeSet::new(),
            target: None,
        }
    }
    /// 记录由浏览器原生 CSS 引擎确认的匹配成员。
    pub fn with_css(mut self, selectors: impl IntoIterator<Item = String>) -> Self {
        self.css.extend(selectors);
        self
    }
    /// 节点的元素或边界类别。
    pub fn kind(&self) -> NodeKind {
        self.kind
    }
    /// 属性缺失表示不可得，不能补空字符串或 false。
    pub fn attributes(&self) -> &BTreeMap<Attribute, Value> {
        &self.attributes
    }
    /// 后端身份；边界根没有动作身份。
    pub fn target(&self) -> Option<&T> {
        self.target.as_ref()
    }
    pub(super) fn css_matches(&self, selector: &str) -> bool {
        self.css.contains(selector)
    }
}
/// 保持来源顺序并限制节点、深度与返回数量的树。
#[derive(Debug)]
pub struct QueryTree<T> {
    nodes: Vec<Node<T>>,
    children: Vec<Vec<usize>>,
    roots: Vec<usize>,
    depths: Vec<usize>,
    max_nodes: usize,
    max_depth: usize,
    max_results: usize,
}
impl<T> QueryTree<T> {
    /// 预算必须非零，节点最多十万、深度最多 256、结果最多 4096。
    pub fn new(max_nodes: usize, max_depth: usize, max_results: usize) -> Result<Self, Failure> {
        if max_nodes == 0
            || max_nodes > 100_000
            || max_depth == 0
            || max_depth > 256
            || max_results == 0
            || max_results > 4096
        {
            return Err(Failure::new(
                FailureKind::InvalidInput,
                "aql_tree",
                "查询树预算无效",
            ));
        }
        Ok(Self {
            nodes: Vec::new(),
            children: Vec::new(),
            roots: Vec::new(),
            depths: Vec::new(),
            max_nodes,
            max_depth,
            max_results,
        })
    }
    /// 插入一个快照节点；每个节点只能有一个已经存在的父节点。
    pub fn push(&mut self, node: Node<T>) -> Result<usize, Failure> {
        let index = self.nodes.len();
        if index >= self.max_nodes {
            return Err(limit());
        }
        let depth = match node.parent {
            Some(parent) => {
                self.depths.get(parent).copied().ok_or_else(|| {
                    Failure::new(FailureKind::Protocol, "aql_tree", "父节点不存在")
                })? + 1
            }
            None => 0,
        };
        if depth > self.max_depth {
            return Err(limit());
        }
        if node.attributes.iter().any(|(attribute, value)| {
            attribute.value_type() != value.value_type() || !value.valid()
        }) {
            return Err(Failure::new(
                FailureKind::Protocol,
                "aql_tree",
                "来源返回无效属性类型或数值",
            ));
        }
        if let Some(parent) = node.parent {
            self.children[parent].push(index);
        } else {
            self.roots.push(index);
        }
        self.nodes.push(node);
        self.children.push(Vec::new());
        self.depths.push(depth);
        Ok(index)
    }
    /// 按节点索引只读访问快照。
    pub fn node(&self, index: usize) -> Option<&Node<T>> {
        self.nodes.get(index)
    }
    /// 已装载节点数。
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    /// 是否没有节点。
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub(super) fn max_results(&self) -> usize {
        self.max_results
    }
    pub(super) fn ordered(
        &self,
        mut selected: BTreeSet<usize>,
        operation: &Operation,
    ) -> Result<Vec<usize>, Failure> {
        let mut pending = self.roots.iter().rev().copied().collect::<Vec<_>>();
        let mut result = Vec::with_capacity(selected.len());
        // 排序包含已经显式打开的边界；只回传 selected，绝不扩大查询范围。
        while let Some(index) = pending.pop() {
            operation.check("aql_order")?;
            if selected.remove(&index) {
                result.push(index);
            }
            pending.extend(self.children[index].iter().rev().copied());
        }
        Ok(result)
    }
    pub(super) fn boundary(&self, host: usize, boundary: Boundary) -> Option<usize> {
        self.children[host].iter().copied().find(|index| matches!(self.nodes[*index].kind, NodeKind::Boundary(value) if value == boundary))
    }
    pub(super) fn candidates(
        &self,
        parent: Option<usize>,
        deep: bool,
        operation: &Operation,
    ) -> Result<Vec<usize>, Failure> {
        let mut pending = parent
            .map(|p| &self.children[p])
            .unwrap_or(&self.roots)
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        let mut result = Vec::new();
        while let Some(index) = pending.pop() {
            operation.check("aql_traversal")?;
            if matches!(self.nodes[index].kind, NodeKind::Boundary(_)) {
                continue;
            }
            result.push(index);
            if deep {
                pending.extend(self.children[index].iter().rev().copied());
            }
        }
        Ok(result)
    }
}
fn limit() -> Failure {
    Failure::new(
        FailureKind::ResourceLimit,
        "aql_tree",
        "查询节点或深度预算耗尽",
    )
}
