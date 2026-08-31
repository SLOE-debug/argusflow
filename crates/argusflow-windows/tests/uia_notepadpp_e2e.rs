//! 真实 Notepad++ provider 上的 process-scoped UIA 查询与 pattern E2E。

#![cfg(windows)]

mod support;

use std::sync::Arc;

use argusflow_agent::{
    AccessibilityContext, ActionBackend, ActionRouter, EvidenceCapturePolicy, EvidenceOutcome,
    EvidenceSettings, ExecutionContext, InMemoryEvidenceSink, PlanStepKind, RuntimeAvailability,
    StaticExecutionContext,
};
use argusflow_core::{AqlQuery, AutomationAction, AutomationError, AutomationTarget, BackendKind};
use argusflow_query::DiagnosticCode;
use argusflow_windows::uia::{UiaBackend, UiaRuntime};
use support::{notepadpp::NotepadPlusPlus, uia_dump::has_value};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires interactive Windows desktop and 64-bit Chinese Notepad++"]
async fn notepadpp_standard_controls_complete_real_uia_e2e() {
    let notepadpp = NotepadPlusPlus::launch();
    let runtime = Arc::new(UiaRuntime::start());
    assert!(runtime.health().is_ready(), "UIA worker should initialize");
    let context = ExecutionContext {
        foreground_window: Some(notepadpp.window()),
        accessibility: AccessibilityContext { ready: true },
        ..ExecutionContext::default()
    };
    let context_provider = Arc::new(StaticExecutionContext::new(context.clone()));
    let backends: Vec<Arc<dyn ActionBackend>> = vec![Arc::new(UiaBackend::new(runtime.clone()))];
    let evidence_sink = Arc::new(InMemoryEvidenceSink::default());
    let router = ActionRouter::with_context_provider(backends, context_provider).with_evidence(
        EvidenceSettings {
            policy: EvidenceCapturePolicy::BranchFailure,
            sink: evidence_sink.clone(),
            ..EvidenceSettings::default()
        },
    );

    // Case 1: compiler、runtime availability 与冻结 Notepad++ HWND 进入同一 PreparedPlan。
    let prepared_action = click(r#"menu_item(name = "搜索(S)")"#);
    let report = router.inspect_current(&prepared_action);
    let prepared_explain = report
        .candidates
        .first()
        .expect("UIA candidate should be explained");
    assert_eq!(report.selected_backend, Some(BackendKind::WindowsUia));
    assert_eq!(prepared_explain.availability, RuntimeAvailability::Ready);

    // Case 2: 按中文可访问名称展开“搜索”菜单，不依赖应用内部命令编号。
    let search_outcome = execute(&router, &context, prepared_action)
        .await
        .expect("中文搜索菜单应公开可展开的 UIA 动作能力");
    assert_eq!(search_outcome.backend, BackendKind::WindowsUia);

    // Case 3: 首分支故意 miss，证据必须先捕获，再由中文名称 fallback 恢复。
    let find_outcome = execute(
        &router,
        &context,
        click(
            r#"any(menu_item(uia.accelerator_key = "__wrong__"), menu_item(name starts_with "查找(F)..."))"#,
        ),
    )
    .await
    .expect("中文查找菜单项应公开 UIA 调用能力");
    assert_eq!(find_outcome.backend, BackendKind::WindowsUia);
    let fallback_evidence = evidence_sink.records();
    assert_eq!(fallback_evidence.len(), 1);
    assert!(matches!(
        fallback_evidence[0].outcome,
        EvidenceOutcome::RecoveredByFallback { .. }
    ));
    assert!(
        fallback_evidence[0]
            .bundle
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == argusflow_agent::EvidenceArtifactKind::SelectorTrace)
    );

    // Case 4: 按中文对话框关系定位输入框，写入必须使用 ValuePattern。
    let expected_value = "argusflow-uia-e2e";
    let set_outcome = execute(
        &router,
        &context,
        set_value(
            r#"dialog(name = "查找") >> textbox(name = "查找目标(F) :")"#,
            expected_value,
        ),
    )
    .await
    .expect("Find input control should expose writable ValuePattern");
    assert_eq!(set_outcome.backend, BackendKind::WindowsUia);
    assert!(
        has_value(&notepadpp.window(), expected_value),
        "ValuePattern write should be readable from the real provider"
    );

    // Case 5: 未显式选择对话框内多个 button 时必须报告歧义，不能偷偷取第一个。
    let ambiguous = execute(
        &router,
        &context,
        click(r#"dialog(name = "查找") >> button()"#),
    )
    .await;
    assert!(matches!(
        ambiguous,
        Err(AutomationError::AmbiguousTarget { matches, .. }) if matches > 1
    ));

    // Case 6: 空结果是 TargetNotFound，不得误报 runtime 不可用触发 backend fallback。
    let missing = execute(
        &router,
        &context,
        click(r#"button(name = "__argusflow_uia_missing_target__")"#),
    )
    .await;
    assert!(matches!(
        missing,
        Err(AutomationError::TargetNotFound { .. })
    ));

    // Case 7: 中文名称 regex 必须走 CacheRequest/residual，再调用“取消”关闭对话框。
    let close_action = click(r#"dialog(name = "查找") >> button(name matches /^取消$/)"#);
    let close_report = router.inspect(&close_action, &context);
    let close_explain = close_report
        .candidates
        .first()
        .expect("regex UIA candidate should be explained");
    assert!(
        close_explain
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::ResidualFilter)
    );
    assert!(
        close_explain
            .steps
            .iter()
            .any(|step| step.kind == PlanStepKind::Cache)
    );
    let close_outcome = execute(&router, &context, close_action)
        .await
        .expect("Find cancel button should expose InvokePattern");
    assert_eq!(close_outcome.backend, BackendKind::WindowsUia);

    let closed_dialog = execute(
        &router,
        &context,
        click(r#"dialog(name = "查找") >> button(name = "取消")"#),
    )
    .await;
    assert!(matches!(
        closed_dialog,
        Err(AutomationError::TargetNotFound { .. })
    ));
}

/// 创建自动选择后端的 AQL Click 动作。
fn click(source: &str) -> AutomationAction {
    AutomationAction::Click {
        target: AutomationTarget::query(AqlQuery::v3(source)),
    }
}

/// 创建自动选择后端的 AQL SetValue 动作。
fn set_value(source: &str, value: &str) -> AutomationAction {
    AutomationAction::SetValue {
        target: AutomationTarget::query(AqlQuery::v3(source)),
        value: value.to_owned(),
    }
}

/// 经过 ActionRouter prepare 后执行冻结计划。
async fn execute(
    router: &ActionRouter,
    context: &ExecutionContext,
    action: AutomationAction,
) -> Result<argusflow_core::ActionOutcome, AutomationError> {
    router.prepare(&action, context)?.execute().await
}
