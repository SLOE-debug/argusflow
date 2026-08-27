use std::sync::Arc;

use argusflow_agent::{ActionBackend, ActionRouter};
use argusflow_browser::{CdpBackend, CdpRuntime};
use argusflow_core::{
    AcquireBrowserSpec, AqlQuery, AutomationAction, AutomationExecutionScope, AutomationTarget,
    BackendKind, BackendPolicy, BrowserAcquireMode, BrowserCleanupPolicy, BrowserSessionProvider,
    TargetScope,
};
use argusflow_runtime::ActionDispatcher;

/// 真实 Chrome、百度 DOM 与持久 CDP actor 的端到端验证。
#[tokio::test]
#[ignore = "requires local Chrome and network access"]
async fn collects_baidu_hot_search_links_with_crlf_records() {
    let executable_path = std::env::var("ARGUSFLOW_CDP_BROWSER_EXE")
        .expect("ARGUSFLOW_CDP_BROWSER_EXE must point to Chrome or Edge");
    let runtime = Arc::new(CdpRuntime::new());
    let session = runtime
        .acquire(&AcquireBrowserSpec {
            executable_path,
            acquire_mode: BrowserAcquireMode::LaunchIsolatedCdp,
            launch_timeout_ms: 15_000,
            cleanup_policy: BrowserCleanupPolicy::CloseOnWorkflowEnd,
        })
        .await
        .expect("browser session should start");
    runtime
        .navigate(&session, "https://www.baidu.com/")
        .await
        .expect("browser should navigate");
    let backend: Arc<dyn ActionBackend> = Arc::new(CdpBackend::new(&runtime));
    let router = ActionRouter::new(vec![backend]);
    let action = AutomationAction::CollectLinks {
        target: AutomationTarget {
            scope: TargetScope::Browser {
                resource: argusflow_core::ResourceRef {
                    producer_node_id: "browser".to_owned(),
                    output_name: "session".to_owned(),
                },
            },
            locator: argusflow_core::TargetLocator::Query {
                query: AqlQuery::v1(
                    r##"css("#hotsearch-content-wrapper a.title-content .title-content-title")"##,
                ),
            },
            backend_policy: BackendPolicy::only(BackendKind::BrowserCdp),
        },
    };

    let outcome = router
        .execute(
            &action,
            AutomationExecutionScope::Browser {
                session_id: session.id,
                target_id: session.target_id.clone(),
            },
        )
        .await
        .expect("CDP should collect links");
    let text = outcome
        .outputs
        .get("text")
        .and_then(serde_json::Value::as_str)
        .expect("collect links should expose text");
    let records = text.strip_suffix("\r\n").expect("text should end in CRLF");

    assert!(!records.is_empty());
    assert!(!records.replace("\r\n", "").contains('\n'));
    assert!(records.split("\r\n").all(|record| {
        let Some((title, url)) = record.split_once('\t') else {
            return false;
        };
        !title.is_empty() && url.starts_with("https://")
    }));

    runtime
        .cleanup(&session)
        .await
        .expect("browser session should clean up");
}
