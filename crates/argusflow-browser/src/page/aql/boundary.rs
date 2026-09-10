//! 显式文档边界装载与自有 OOPIF 会话释放。
use super::super::handle::{protocol, stale};
use super::{context::DocumentContext, model::BrowserMatch, snapshot};
use crate::{BrowserError, Page, cdp::Cleanup};
use argusflow_aql::{BoundQuery, Boundary, Node, QueryTree};
use argusflow_core::{FailureKind, Operation};
use serde_json::json;
use std::sync::{Arc, atomic::Ordering};

pub(crate) struct FrameSession {
    page: Page,
    _cleanup: Cleanup,
}
async fn attach(
    host: &BrowserMatch,
    frame_id: &str,
    operation: &Operation,
) -> Result<Arc<FrameSession>, BrowserError> {
    let parent = &host.context.page;
    let mut sessions = parent.inner.aql_frames.try_lock().map_err(|_| {
        BrowserError::new(FailureKind::Busy, "aql_frame_attach", "正在附加子 frame")
    })?;
    sessions.retain(|_, session| session.strong_count() > 0);
    if let Some(session) = sessions.get(frame_id).and_then(|session| session.upgrade()) {
        return Ok(session);
    }
    if sessions.len() >= 64 {
        return Err(BrowserError::new(
            FailureKind::ResourceLimit,
            "aql_frame_attach",
            "子 frame 会话超过 64 个",
        ));
    }
    // Chromium 的 OOPIF targetId 与 frameId 对应；附加后再次核对 frame 树，防止目标错误绑定。
    let targets = parent
        .inner
        .connection
        .command(None, "Target.getTargets", json!({}), false, operation)
        .await?;
    let exists = targets["targetInfos"].as_array().is_some_and(|targets| {
        targets.iter().any(|target| {
            target["targetId"].as_str() == Some(frame_id) && target["type"] == "iframe"
        })
    });
    if !exists {
        return Err(BrowserError::new(
            FailureKind::StaleHandle,
            "aql_frame_attach",
            "目标 iframe 的进程会话尚不可用或已经切换",
        ));
    }
    let permit = parent
        .inner
        .connection
        .inner
        .resources
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            BrowserError::new(FailureKind::Busy, "aql_frame_attach", "会话清理额度已满")
        })?;
    let page = Page::attach(parent.inner.connection.clone(), frame_id, operation).await?;
    let mut cleanup = Cleanup::new(parent.inner.connection.clone(), None, permit);
    cleanup.commands.push((
        "Target.detachFromTarget",
        json!({"sessionId":page.inner.session}),
    ));
    let info = page
        .command("Page.getFrameTree", json!({}), false, operation)
        .await?;
    if info["frameTree"]["frame"]["id"].as_str() != Some(frame_id) {
        return Err(protocol("子会话的文档身份与 iframe 不一致"));
    }
    host.context.check()?;
    let session = Arc::new(FrameSession {
        page,
        _cleanup: cleanup,
    });
    sessions.insert(frame_id.into(), Arc::downgrade(&session));
    Ok(session)
}
pub(super) async fn load(
    tree: &mut QueryTree<BrowserMatch>,
    host_index: usize,
    boundary: Boundary,
    query: &BoundQuery,
    operation: &Operation,
) -> Result<(), BrowserError> {
    let host = tree
        .node(host_index)
        .and_then(|node| node.target())
        .cloned()
        .ok_or_else(stale)?;
    host.context.check()?;
    let (root, context) = match boundary {
        Boundary::Shadow => {
            let root = host
                .node
                .shadow_roots
                .iter()
                .find(|root| root.shadow_root_type.as_deref() == Some("open"))
                .cloned()
                .ok_or_else(|| {
                    BrowserError::new(
                        FailureKind::Unsupported,
                        "aql_shadow",
                        "宿主没有开放的 Shadow Root",
                    )
                })?;
            (root, host.context.clone())
        }
        Boundary::Frame => {
            if host.node.local_name != "iframe" && host.node.local_name != "frame" {
                return Err(BrowserError::new(
                    FailureKind::InvalidInput,
                    "aql_frame",
                    "frame 的宿主必须是 iframe 元素",
                ));
            }
            let frame_id = host
                .node
                .frame_id
                .clone()
                .or_else(|| {
                    host.node
                        .content_document
                        .as_ref()
                        .and_then(|root| root.frame_id.clone())
                })
                .ok_or_else(|| protocol("iframe 没有 frameId"))?;
            let (root, page, lease) = if let Some(root) = &host.node.content_document {
                (root.clone(), host.context.page.clone(), None)
            } else {
                let session = attach(&host, &frame_id, operation).await?;
                let root = snapshot::document(&session.page, operation).await?;
                (root, session.page.clone(), Some(session))
            };
            let epoch = page.inner.state.epoch.load(Ordering::Acquire);
            let context = Arc::new(DocumentContext {
                page,
                epoch,
                frame_id,
                owner: Some(host.clone()),
                _lease: lease,
            });
            (root, context)
        }
    };
    let boundary_root = tree.push(Node::boundary(host_index, boundary))?;
    snapshot::append(tree, root, context, Some(boundary_root), query, operation).await?;
    host.context.check()?;
    Ok(())
}
