//! 真实 Notepad++ provider 上的 HWND scoped UIA 查询与 pattern E2E。

#![cfg(windows)]

mod support;

use std::{sync::Arc, time::Duration};

use argusflow_agent::{
    AccessibilityContext, ActionBackend, ActionRouter, ExecutionContext, PlanStepKind,
    RuntimeAvailability, StaticExecutionContext,
};
use argusflow_core::{AqlQuery, AutomationAction, AutomationError, AutomationTarget, BackendKind};
use argusflow_query::DiagnosticCode;
use argusflow_windows::uia::{UiaBackend, UiaRuntime};
use support::{notepadpp::NotepadPlusPlus, uia_dump::has_value};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires interactive Windows desktop and 64-bit English Notepad++"]
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
    let router = ActionRouter::with_context_provider(backends, context_provider);

    // Case 1: compiler、runtime availability 与冻结 Notepad++ HWND 进入同一 PreparedPlan。
    let prepared_action =
        click(r#"window(name contains "Notepad++") >> menu_item(name = "Search")"#);
    let report = router.inspect_current(&prepared_action);
    let prepared_explain = report
        .candidates
        .first()
        .expect("UIA candidate should be explained");
    assert_eq!(report.selected_backend, Some(BackendKind::WindowsUia));
    assert_eq!(prepared_explain.availability, RuntimeAvailability::Ready);

    // Case 2: Search 菜单项必须通过 InvokePattern 打开，禁止物理输入回退。
    let search_outcome = execute(
        &router,
        &context,
        click(r#"window(name contains "Notepad++") >> menu_item(name = "Search")"#),
    )
    .await
    .expect("Search menu item should expose InvokePattern");
    assert_eq!(search_outcome.backend, BackendKind::WindowsUia);
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Case 3: 从已展开菜单调用 Find，并等待真实 Find dialog provider 出现。
    let find_outcome = execute(
        &router,
        &context,
        click(r#"first(menu_item(name starts_with "Find"))"#),
    )
    .await
    .expect("Find menu item should expose InvokePattern");
    assert_eq!(find_outcome.backend, BackendKind::WindowsUia);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Case 4: Edit/ComboBox provider 差异由 AQL any 表达，写入必须使用 ValuePattern。
    let expected_value = "argusflow-uia-e2e";
    let set_outcome = execute(
        &router,
        &context,
        set_value(
            r#"dialog(name contains "Find") >> first(any(textbox(name contains "Find what"), combobox(name contains "Find what")))"#,
            expected_value,
        ),
    )
    .await
    .expect("Find what control should expose writable ValuePattern");
    assert_eq!(set_outcome.backend, BackendKind::WindowsUia);
    assert!(
        has_value(&notepadpp.window(), expected_value),
        "ValuePattern write should be readable from the real provider"
    );

    // Case 6: 未显式选择多个 button 时必须报告歧义，不能偷偷取第一个。
    let ambiguous = execute(
        &router,
        &context,
        click(r#"dialog(name contains "Find") >> button()"#),
    )
    .await;
    assert!(matches!(
        ambiguous,
        Err(AutomationError::AmbiguousTarget { matches, .. }) if matches > 1
    ));

    // Case 7: 空结果是 TargetNotFound，不得误报 runtime 不可用触发 backend fallback。
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

    // Case 8 + Case 5: regex 必须走 CacheRequest/residual，再用 InvokePattern 关闭对话框。
    let close_action =
        click(r#"dialog(name contains "Find") >> any(button(name matches /Close/i), button())"#);
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
        .expect("Close button should expose InvokePattern");
    assert_eq!(close_outcome.backend, BackendKind::WindowsUia);
    tokio::time::sleep(Duration::from_millis(300)).await;

    let closed_dialog = execute(
        &router,
        &context,
        click(r#"dialog(name contains "Find") >> button(name matches /Close/i)"#),
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
        target: AutomationTarget::query(AqlQuery::v1(source)),
    }
}

/// 创建自动选择后端的 AQL SetValue 动作。
fn set_value(source: &str, value: &str) -> AutomationAction {
    AutomationAction::SetValue {
        target: AutomationTarget::query(AqlQuery::v1(source)),
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
