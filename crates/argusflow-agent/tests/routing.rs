//! ActionRouter 的 capability/cost 排序与显式后端偏好测试。

use std::sync::Arc;

use argusflow_agent::{ActionBackend, ActionCapability, ActionRouter};
use argusflow_core::{
    ActionOutcome, AqlQuery, AutomationAction, AutomationError, AutomationTarget, BackendKind,
    BackendPreference,
};
use argusflow_query::{QueryCost, SupportLevel};
use argusflow_runtime::ActionDispatcher;
use async_trait::async_trait;

/// 返回固定计划和成功结果的路由测试后端。
struct PlannedBackend {
    /// 后端类别。
    kind: BackendKind,
    /// 路由时返回的能力。
    capability: ActionCapability,
}

#[async_trait]
impl ActionBackend for PlannedBackend {
    fn kind(&self) -> BackendKind {
        self.kind
    }

    fn plan(&self, _action: &AutomationAction) -> ActionCapability {
        self.capability
    }

    async fn execute(&self, _action: &AutomationAction) -> Result<ActionOutcome, AutomationError> {
        Ok(ActionOutcome {
            backend: self.kind,
            message: "planned backend executed".to_owned(),
        })
    }
}

#[tokio::test]
async fn router_prefers_better_support_level_over_static_backend_order() {
    let router = ActionRouter::new(vec![
        Arc::new(PlannedBackend {
            kind: BackendKind::WindowsUia,
            capability: ActionCapability {
                level: SupportLevel::Hybrid,
                estimated_cost: QueryCost::Medium,
            },
        }),
        Arc::new(PlannedBackend {
            kind: BackendKind::BrowserCdp,
            capability: ActionCapability {
                level: SupportLevel::Native,
                estimated_cost: QueryCost::Low,
            },
        }),
    ]);

    let outcome = router
        .execute(&portable_click())
        .await
        .expect("a planned backend should execute");

    assert_eq!(outcome.backend, BackendKind::BrowserCdp);
}

#[tokio::test]
async fn router_honors_backend_preference_without_mutating_query() {
    let router = ActionRouter::new(vec![
        Arc::new(PlannedBackend {
            kind: BackendKind::WindowsUia,
            capability: ActionCapability {
                level: SupportLevel::Native,
                estimated_cost: QueryCost::Low,
            },
        }),
        Arc::new(PlannedBackend {
            kind: BackendKind::BrowserCdp,
            capability: ActionCapability {
                level: SupportLevel::Hybrid,
                estimated_cost: QueryCost::Medium,
            },
        }),
    ]);
    let mut action = portable_click();
    let AutomationAction::Click { target } = &mut action else {
        unreachable!("test action is always Click");
    };
    target.backend_preference = BackendPreference::BrowserCdp;

    let outcome = router
        .execute(&action)
        .await
        .expect("forced CDP backend should execute");

    assert_eq!(outcome.backend, BackendKind::BrowserCdp);
}

/// 构造默认自动规划的 portable AQL 点击动作。
fn portable_click() -> AutomationAction {
    AutomationAction::Click {
        target: AutomationTarget::query(AqlQuery::v1("button(name = \"保存\")")),
    }
}
