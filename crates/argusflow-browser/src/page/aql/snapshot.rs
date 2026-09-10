//! DOM 身份与同根脚本、AX 快照以文档代际关联。
use super::super::handle::{protocol, stale};
use super::{
    context::DocumentContext,
    model::{AxResponse, BrowserMatch, DomFacts, DomNode, DomResponse},
    properties,
};
use crate::{BrowserError, Page};
use argusflow_aql::{BoundQuery, Node, QueryTree};
use argusflow_core::{FailureKind, Operation};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};

pub(super) async fn document(
    page: &Page,
    operation: &Operation,
) -> Result<Arc<DomNode>, BrowserError> {
    let epoch = page.inner.state.epoch.load(Ordering::Acquire);
    let mut cache = page.inner.document.try_lock().map_err(|_| {
        BrowserError::new(
            FailureKind::Busy,
            "aql_document",
            "当前页面正在建立 DOM 身份",
        )
    })?;
    let result = if let Some((generation, root)) = *cache
        && generation == epoch
    {
        page.command(
            "DOM.describeNode",
            json!({"nodeId":root,"depth":-1,"pierce":true}),
            false,
            operation,
        )
        .await?
    } else {
        page.command(
            "DOM.getDocument",
            json!({"depth":-1,"pierce":true}),
            false,
            operation,
        )
        .await?
    };
    let response: DomResponse =
        serde_json::from_value(result).map_err(|e| protocol("DOM 文档格式错误").with_source(e))?;
    let root = response
        .root
        .or(response.node)
        .ok_or_else(|| protocol("DOM 响应缺少根"))?;
    if epoch != page.inner.state.epoch.load(Ordering::Acquire) {
        return Err(stale());
    }
    *cache = Some((epoch, root.node_id));
    Ok(root)
}
pub(super) async fn append(
    tree: &mut QueryTree<BrowserMatch>,
    root: Arc<DomNode>,
    context: Arc<DocumentContext>,
    parent: Option<usize>,
    query: &BoundQuery,
    operation: &Operation,
) -> Result<(), BrowserError> {
    context.check()?;
    let response = context
        .page
        .command(
            "Accessibility.getFullAXTree",
            json!({"frameId":context.frame_id}),
            false,
            operation,
        )
        .await?;
    let response: AxResponse =
        serde_json::from_value(response).map_err(|e| protocol("AX 响应格式错误").with_source(e))?;
    if response.nodes.len() > 10000 {
        return Err(BrowserError::new(
            FailureKind::ResourceLimit,
            "aql_ax",
            "AX 节点预算耗尽",
        ));
    }
    let ax = response
        .nodes
        .into_iter()
        .filter_map(|node| node.backend_dom_node_id.map(|id| (id, node)))
        .collect::<HashMap<_, _>>();
    let facts = context
        .call(
            root.backend_node_id,
            include_str!("snapshot.js"),
            vec![json!(query.css_selectors())],
            false,
            operation,
        )
        .await?;
    let facts: Vec<DomFacts> = serde_json::from_value(facts)
        .map_err(|e| protocol("DOM 属性快照格式错误").with_source(e))?;
    if facts.len() > 10000 {
        return Err(BrowserError::new(
            FailureKind::ResourceLimit,
            "aql_dom",
            "DOM 节点预算耗尽",
        ));
    }
    let mut facts = facts
        .into_iter()
        .map(|facts| (facts.path.clone(), facts))
        .collect::<HashMap<_, _>>();
    let mut pending = vec![(root, String::new(), parent)];
    let mut visited = 0;
    while let Some((node, path, parent)) = pending.pop() {
        operation.check("aql_dom_snapshot")?;
        visited += 1;
        if visited > 10000 {
            return Err(BrowserError::new(
                FailureKind::ResourceLimit,
                "aql_dom",
                "DOM 节点预算耗尽",
            ));
        }
        let parent = if matches!(node.node_type, 1 | 3 | 9) {
            let facts = facts.remove(&path).ok_or_else(stale)?;
            let semantic = ax.get(&node.backend_node_id);
            let role = properties::role(semantic);
            let attributes = properties::attributes(&facts, semantic, &node.local_name);
            let target = BrowserMatch {
                context: context.clone(),
                node: node.clone(),
                role,
                attributes: attributes.clone(),
                bounds: facts.bounds,
            };
            Some(tree.push(Node::element(parent, role, attributes, target).with_css(facts.css))?)
        } else {
            parent
        };
        pending.extend(
            node.children
                .iter()
                .enumerate()
                .rev()
                .map(|(index, child)| (child.clone(), format!("{path}/{index}"), parent)),
        );
    }
    if !facts.is_empty() {
        return Err(stale());
    }
    context.check()?;
    Ok(())
}
