//! 平台查询入口只做能力检查、按需采集及共享求值编排。
use super::super::handle::protocol;
use super::{boundary, context::DocumentContext, model::BrowserMatch, snapshot};
use crate::{BrowserError, Page};
use argusflow_aql::{
    Attribute, BoundQuery, Capabilities, QueryProgress, QueryTree, Role, evaluate_step,
};
use argusflow_core::{FailureKind, Operation};
use serde_json::json;
use std::sync::{Arc, atomic::Ordering};

impl Page {
    /// 在当前页面执行 AQL；普通关系不穿透 iframe 和 Shadow Root。
    pub async fn query_aql(
        &self,
        query: &BoundQuery,
        operation: &Operation,
    ) -> Result<Vec<BrowserMatch>, BrowserError> {
        let attributes = Attribute::ALL.iter().copied().filter(|attribute| {
            !matches!(
                attribute,
                Attribute::AutomationId
                    | Attribute::ClassName
                    | Attribute::AcceleratorKey
                    | Attribute::AccessKey
                    | Attribute::FrameworkId
                    | Attribute::Confidence
            )
        });
        Capabilities::new(
            Role::ALL
                .iter()
                .copied()
                .filter(|role| *role != Role::Window),
            attributes,
        )
        .with_relations()
        .with_browser_boundaries()
        .check(query)?;
        operation.check("aql_browser_prepare")?;
        let mut guard = operation.cancel_on_drop();
        let info = self
            .command("Page.getFrameTree", json!({}), false, operation)
            .await?;
        let frame_id = info["frameTree"]["frame"]["id"]
            .as_str()
            .ok_or_else(|| protocol("主页面缺少 frameId"))?
            .to_owned();
        let context = Arc::new(DocumentContext {
            page: self.clone(),
            epoch: self.inner.state.epoch.load(Ordering::Acquire),
            frame_id,
            owner: None,
            _lease: None,
        });
        let root = snapshot::document(self, operation).await?;
        let mut tree = QueryTree::new(10000, 64, self.inner.connection.inner.config.max_elements)?;
        snapshot::append(&mut tree, root, context.clone(), None, query, operation).await?;
        for _ in 0..=64 {
            match evaluate_step(query, &tree, operation)? {
                QueryProgress::Complete(indices) => {
                    context.check()?;
                    let results = indices
                        .into_iter()
                        .filter_map(|index| {
                            tree.node(index).and_then(|node| node.target()).cloned()
                        })
                        .collect::<Vec<_>>();
                    for result in &results {
                        result.context.check()?;
                    }
                    guard.disarm();
                    return Ok(results);
                }
                QueryProgress::Boundary {
                    host,
                    boundary: kind,
                } => boundary::load(&mut tree, host, kind, query, operation).await?,
            }
        }
        Err(BrowserError::new(
            FailureKind::ResourceLimit,
            "aql_browser",
            "查询文档边界超过 64 个",
        ))
    }
}
