//! 单次反查 CDP remote object group 的取消安全生命周期。

use crate::cdp::CdpPageSession;
use serde_json::json;
use std::{sync::Arc, time::Duration};

/// 即使 resolver deadline 取消检查，也异步释放此次反查产生的 remote handles。
pub(super) struct InspectionLease {
    /// 只持有当前已附加 page，不创建新连接。
    page: Arc<CdpPageSession>,
    /// 每次检查唯一，避免并发检查相互释放对象。
    group: String,
}

impl InspectionLease {
    pub(super) fn new(page: Arc<CdpPageSession>) -> Self {
        Self {
            page,
            group: format!("argusflow-inspection-{}", uuid::Uuid::new_v4()),
        }
    }

    pub(super) fn group(&self) -> &str {
        &self.group
    }
}

impl Drop for InspectionLease {
    fn drop(&mut self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let page = self.page.clone();
        let group = self.group.clone();
        runtime.spawn(async move {
            let _ = tokio::time::timeout(
                Duration::from_secs(1),
                page.command(
                    "Runtime.releaseObjectGroup",
                    json!({ "objectGroup": group }),
                ),
            )
            .await;
        });
    }
}
