use std::sync::Arc;

use argusflow_agent::{
    ActionBackend, ActionRouter, ExecutionContext, RuntimeAvailability, WindowContext,
};
use argusflow_core::{
    AutomationAction, AutomationError, AutomationTarget, BackendKind, BackendPolicy,
    PreparedAutomationTarget, PreparedTargetLocator, TargetLocator, TargetScope, ValueExpr,
    VisualQuery, VisualQueryExpr,
};
use argusflow_vision::{
    MemoryFrameSource, OcrResponse, StaticOcrEngine, UnavailableOcrEngine, VisionBackend,
    VisionRuntime, VisionWorkerClient,
};

/// 创建尚未产生 scene 的视觉运行时。
fn empty_runtime(worker: Arc<VisionWorkerClient>) -> Arc<VisionRuntime> {
    Arc::new(VisionRuntime::new(
        Arc::new(MemoryFrameSource::new()),
        worker,
    ))
}

/// 创建同时允许 cache 与 tiny OCR 的读取动作及其冻结目标。
fn visual_get_text() -> (AutomationAction, PreparedAutomationTarget) {
    visual_get_text_with_policy(BackendPolicy {
        allow: vec![BackendKind::VisualCache, BackendKind::OcrTiny],
        deny: Vec::new(),
        prefer: Vec::new(),
    })
}

/// 创建使用指定后端策略的视觉读取动作及其冻结目标。
fn visual_get_text_with_policy(
    backend_policy: BackendPolicy,
) -> (AutomationAction, PreparedAutomationTarget) {
    let query = VisualQuery {
        text: "搜索".to_owned(),
        exact: true,
        region: None,
    };
    let target = AutomationTarget {
        scope: TargetScope::Current,
        locator: TargetLocator::Visual {
            query: VisualQueryExpr {
                text: ValueExpr::text(query.text.clone()),
                exact: query.exact,
                region: query.region,
            },
        },
        backend_policy: backend_policy.clone(),
    };
    let prepared_target = PreparedAutomationTarget::new(
        TargetScope::Current,
        PreparedTargetLocator::Visual { query },
        backend_policy,
    );
    (AutomationAction::GetText { target }, prepared_target)
}

#[test]
fn automatic_desktop_ocr_prefers_small_over_tiny() {
    let worker = Arc::new(VisionWorkerClient::new(Arc::new(StaticOcrEngine::new(
        Vec::<OcrResponse>::new(),
    ))));
    let runtime = empty_runtime(worker);
    let router = ActionRouter::new(vec![
        Arc::new(VisionBackend::new(runtime.clone(), BackendKind::OcrTiny)),
        Arc::new(VisionBackend::new(runtime, BackendKind::OcrSmall)),
    ]);
    let (action, prepared_target) = visual_get_text_with_policy(BackendPolicy::default());

    let plan = router
        .prepare_with_target(&action, &window_context(), Some(&prepared_target))
        .expect("ready desktop OCR should choose its balanced default tier");

    assert_eq!(plan.selected_backend(), BackendKind::OcrSmall);
}

/// 创建具备冻结窗口身份的规划上下文。
fn window_context() -> ExecutionContext {
    ExecutionContext {
        foreground_window: Some(WindowContext {
            handle: 11,
            process_id: 22,
        }),
        ..ExecutionContext::default()
    }
}

#[test]
fn empty_cache_is_unavailable_instead_of_claiming_a_ready_candidate() {
    let worker = Arc::new(VisionWorkerClient::new(Arc::new(
        UnavailableOcrEngine::new("worker is intentionally unavailable"),
    )));
    let backend = VisionBackend::new(empty_runtime(worker), BackendKind::VisualCache);
    let (action, prepared_target) = visual_get_text();

    let candidates = backend
        .prepare_with_target(&action, &window_context(), Some(&prepared_target))
        .expect("visual cache backend should explain its availability");

    assert_eq!(
        candidates[0].explain().availability,
        RuntimeAvailability::Unavailable
    );
}

#[test]
fn ready_ocr_is_selected_to_seed_an_empty_visual_cache() {
    let worker = Arc::new(VisionWorkerClient::new(Arc::new(StaticOcrEngine::new(
        Vec::<OcrResponse>::new(),
    ))));
    let runtime = empty_runtime(worker);
    let router = ActionRouter::new(vec![
        Arc::new(VisionBackend::new(
            runtime.clone(),
            BackendKind::VisualCache,
        )),
        Arc::new(VisionBackend::new(runtime, BackendKind::OcrTiny)),
    ]);
    let (action, prepared_target) = visual_get_text();

    let plan = router
        .prepare_with_target(&action, &window_context(), Some(&prepared_target))
        .expect("ready tiny OCR should seed an empty visual cache");

    assert_eq!(plan.selected_backend(), BackendKind::OcrTiny);
}

#[test]
fn empty_cache_and_unavailable_worker_leave_no_executable_backend() {
    let worker = Arc::new(VisionWorkerClient::new(Arc::new(
        UnavailableOcrEngine::new("worker is intentionally unavailable"),
    )));
    let runtime = empty_runtime(worker);
    let router = ActionRouter::new(vec![
        Arc::new(VisionBackend::new(
            runtime.clone(),
            BackendKind::VisualCache,
        )),
        Arc::new(VisionBackend::new(runtime, BackendKind::OcrTiny)),
    ]);
    let (action, prepared_target) = visual_get_text();

    let result = router.prepare_with_target(&action, &window_context(), Some(&prepared_target));

    assert!(matches!(result, Err(AutomationError::NoBackendAvailable)));
}
