//! 真实 Notepad++ 进程的启动、最小化恢复与窗口身份验收。

mod support;

use std::{
    env, fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use argusflow_agent::{
    AccessibilityContext, ActionBackend, ActionRouter, ExecutionContext, StaticExecutionContext,
};
use argusflow_core::{
    ApplicationTarget, AqlQuery, AutomationAction, AutomationTarget, BackendKind,
    BackendPreference, ExecutionEvent, ExecutionEventKind, Position, TargetLocator,
    WindowTitleMatcher, WorkflowDefinition, WorkflowEdge, WorkflowNode, WorkflowNodeKind,
};
use argusflow_runtime::{ExecutionEventSink, WorkflowEngine};
use argusflow_windows::{
    uia::{UiaBackend, UiaRuntime},
    window::{ResolvedWindow, WindowService},
};
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, PostMessageW, SW_MINIMIZE, ShowWindowAsync, WM_CLOSE,
    },
};
use windows::core::BOOL;

use support::uia_dump::has_name_for_process;

/// 只在显式提供 Notepad++ EXE 时运行，避免普通测试意外启动桌面程序。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an interactive Windows desktop and ARGUSFLOW_NOTEPADPP_EXE"]
async fn launches_then_restores_the_same_notepadpp_window() {
    let (target, document) = create_target("window-lifecycle");
    let service = WindowService;
    let first = service
        .resolve_application(target.clone())
        .await
        .expect("resolver should launch a unique Notepad++ window");
    let cleanup = WindowCleanup {
        window: first,
        document,
    };

    let native = HWND(first.handle as usize as *mut std::ffi::c_void);
    // SAFETY: HWND 由刚完成的 WindowService 枚举返回，只请求最小化该测试窗口。
    let _ = unsafe { ShowWindowAsync(native, SW_MINIMIZE) };
    tokio::time::sleep(Duration::from_millis(300)).await;
    // SAFETY: 测试仍拥有该窗口的生命周期，调用只读取最小化状态。
    assert!(unsafe { IsIconic(native) }.as_bool());

    let restored = service
        .resolve_application(target)
        .await
        .expect("resolver should restore the existing Notepad++ window");
    assert_eq!(restored, first);
    // SAFETY: restored HWND 已由 WindowService 再次枚举和激活。
    assert!(!unsafe { IsIconic(native) }.as_bool());

    drop(cleanup);
}

/// 产品完整路径必须启动 Notepad++、执行真实 UIA Invoke 并产生可观察菜单状态。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an interactive Windows desktop and 64-bit English Notepad++"]
async fn workflow_application_query_invokes_notepadpp_and_changes_observable_ui() {
    let (application, document) = create_target("uia-invoke");
    let runtime = Arc::new(UiaRuntime::start());
    assert!(runtime.health().is_ready(), "UIA worker should initialize");
    let context = ExecutionContext {
        accessibility: AccessibilityContext { ready: true },
        ..ExecutionContext::default()
    };
    let context_provider = Arc::new(StaticExecutionContext::new(context.clone()));
    let backends: Vec<Arc<dyn ActionBackend>> = vec![Arc::new(UiaBackend::new(runtime))];
    let router = Arc::new(ActionRouter::with_context_provider(
        backends,
        context_provider,
    ));
    let action = AutomationAction::Click {
        target: AutomationTarget {
            locator: TargetLocator::ApplicationQuery {
                application: application.clone(),
                query: AqlQuery::v1(
                    r#"first(window(name contains "Notepad++") >> menu_item(name = "?"))"#,
                ),
            },
            backend_preference: BackendPreference::WindowsUia,
        },
    };
    let planning = router.inspect(&action, &context);
    assert_eq!(planning.selected_backend, Some(BackendKind::WindowsUia));
    let engine = Arc::new(WorkflowEngine::new(router));
    let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
    engine
        .start(
            application_workflow(action),
            Arc::new(ChannelSink(event_sender)),
        )
        .await
        .expect("complete application workflow should be accepted");
    let events = tokio::time::timeout(Duration::from_secs(40), async move {
        let mut events = Vec::new();
        while let Some(event) = event_receiver.recv().await {
            let finished = matches!(
                event.kind,
                ExecutionEventKind::WorkflowCompleted | ExecutionEventKind::WorkflowFailed
            );
            events.push(event);
            if finished {
                break;
            }
        }
        events
    })
    .await
    .expect("workflow should finish within the E2E timeout");
    let window = wait_for_test_window(application.window_title.value(), Duration::from_secs(2))
        .expect("application query should leave its uniquely titled Notepad++ window running");
    let cleanup = WindowCleanup { window, document };
    assert_eq!(
        events.last().map(|event| event.kind),
        Some(ExecutionEventKind::WorkflowCompleted)
    );
    assert!(events.iter().any(|event| {
        event.kind == ExecutionEventKind::Log
            && event
                .message
                .as_deref()
                .is_some_and(|message| message.contains("InvokePattern"))
    }));
    assert!(
        has_name_for_process(window.process_id, "About Notepad++"),
        "Help menu invocation should expose the real About Notepad++ menu item"
    );

    drop(cleanup);
}

/// 只读等待测试创建的唯一标题窗口，避免再次激活主 HWND 导致已展开菜单关闭。
fn wait_for_test_window(title_fragment: &str, timeout: Duration) -> Option<ResolvedWindow> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(window) = find_test_window(title_fragment) {
            return Some(window);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

/// 按随机临时文件名枚举本测试的唯一可见顶层窗口。
fn find_test_window(title_fragment: &str) -> Option<ResolvedWindow> {
    let mut search = TestWindowSearch {
        title_fragment,
        window: None,
    };
    let parameter = LPARAM((&mut search as *mut TestWindowSearch<'_>) as isize);
    // SAFETY: callback 与 parameter 在同步 EnumWindows 调用期间保持有效。
    let _ = unsafe { EnumWindows(Some(enum_test_window), parameter) };
    search.window
}

/// `EnumWindows` callback 使用的只读标题条件与首个匹配结果。
struct TestWindowSearch<'a> {
    /// 每个 E2E 临时文档独有的窗口标题片段。
    title_fragment: &'a str,
    /// 找到后立即停止枚举的 HWND/PID。
    window: Option<ResolvedWindow>,
}

/// 枚举可见标题窗口并冻结 HWND/PID，匹配成功后提前停止。
unsafe extern "system" fn enum_test_window(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: parameter 由 find_test_window 传入，同步 EnumWindows 返回前始终有效。
    let search = unsafe { &mut *(parameter.0 as *mut TestWindowSearch<'_>) };
    // SAFETY: window 由系统枚举提供，只执行只读可见性检查。
    if !unsafe { IsWindowVisible(window) }.as_bool() {
        return true.into();
    }
    // SAFETY: window 仍是当前同步 callback 提供的有效枚举值，只读取标题长度。
    let title_length = unsafe { GetWindowTextLengthW(window) };
    if title_length <= 0 {
        return true.into();
    }
    let mut title = vec![0_u16; title_length as usize + 1];
    // SAFETY: title buffer 可写且为结尾留有空间，window 在同步 callback 中有效。
    let copied = unsafe { GetWindowTextW(window, &mut title) };
    let title = String::from_utf16_lossy(&title[..copied.max(0) as usize]);
    if !title
        .to_lowercase()
        .contains(&search.title_fragment.to_lowercase())
    {
        return true.into();
    }
    let mut process_id = 0_u32;
    // SAFETY: process_id 指针在同步调用期间有效且独占。
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    search.window = Some(ResolvedWindow {
        handle: window.0 as usize as u64,
        process_id,
    });
    false.into()
}

/// 真实工作流 E2E 使用的内存事件接收器。
struct ChannelSink(mpsc::UnboundedSender<ExecutionEvent>);

impl ExecutionEventSink for ChannelSink {
    /// 把 runtime 生命周期事件转发给测试任务。
    fn emit(&self, event: ExecutionEvent) -> Result<(), String> {
        self.0.send(event).map_err(|error| error.to_string())
    }
}

/// 构造 Start -> ApplicationQuery Action -> End 的产品完整执行路径。
fn application_workflow(action: AutomationAction) -> WorkflowDefinition {
    WorkflowDefinition {
        schema_version: 3,
        id: Uuid::new_v4(),
        name: "Notepad++ ApplicationQuery E2E".to_owned(),
        variables: json!({}),
        nodes: vec![
            WorkflowNode {
                id: "start".to_owned(),
                position: Position { x: 0.0, y: 0.0 },
                kind: WorkflowNodeKind::Start,
            },
            WorkflowNode {
                id: "invoke-help".to_owned(),
                position: Position { x: 220.0, y: 0.0 },
                kind: WorkflowNodeKind::Action { action },
            },
            WorkflowNode {
                id: "end".to_owned(),
                position: Position { x: 440.0, y: 0.0 },
                kind: WorkflowNodeKind::End,
            },
        ],
        edges: vec![
            WorkflowEdge {
                id: "start-invoke".to_owned(),
                source: "start".to_owned(),
                target: "invoke-help".to_owned(),
                branch: None,
            },
            WorkflowEdge {
                id: "invoke-end".to_owned(),
                source: "invoke-help".to_owned(),
                target: "end".to_owned(),
                branch: None,
            },
        ],
    }
}

/// 为每个测试创建标题唯一的 Notepad++ 文档和显式应用契约。
fn create_target(scenario: &str) -> (ApplicationTarget, PathBuf) {
    let executable = env::var_os("ARGUSFLOW_NOTEPADPP_EXE")
        .map(PathBuf::from)
        .expect("ARGUSFLOW_NOTEPADPP_EXE must point to notepad++.exe");
    let document = env::temp_dir().join(format!("argusflow-{scenario}-{}.txt", std::process::id()));
    fs::write(&document, "ArgusFlow application lifecycle E2E")
        .expect("temporary Notepad++ document should be writable");
    let title_fragment = document
        .file_name()
        .and_then(|name| name.to_str())
        .expect("temporary document should have a UTF-8 file name")
        .to_owned();
    let target = ApplicationTarget {
        executable_path: executable.to_string_lossy().into_owned(),
        arguments: vec![
            "-multiInst".to_owned(),
            "-nosession".to_owned(),
            "-noPlugin".to_owned(),
            document.to_string_lossy().into_owned(),
        ],
        window_title: WindowTitleMatcher::Contains {
            value: title_fragment,
        },
        launch_timeout_ms: 10_000,
    };
    (target, document)
}

/// 无论断言是否完成，都关闭测试创建的窗口并删除精确临时文件。
struct WindowCleanup {
    /// 测试创建的唯一 Notepad++ 窗口。
    window: ResolvedWindow,
    /// 测试创建且只由本 guard 删除的临时文档。
    document: PathBuf,
}

impl Drop for WindowCleanup {
    fn drop(&mut self) {
        let native = HWND(self.window.handle as usize as *mut std::ffi::c_void);
        // SAFETY: HWND 来自本测试启动的唯一标题窗口；消息不携带外部指针。
        let _ = unsafe { PostMessageW(Some(native), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        let _ = fs::remove_file(&self.document);
    }
}
