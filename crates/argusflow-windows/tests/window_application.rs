//! 真实 Notepad++ AppSession 的启动、复用、恢复与 Workflow 作用域验收。

mod support;

use std::{
    env, fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use argusflow_agent::{ActionBackend, ActionRouter};
use argusflow_core::{
    AcquirePolicy, ActivationPolicy, ApplicationSessionProvider, ApplicationSpec,
    BackendPreference, CleanupPolicy, ExecutionEvent, ExecutionEventKind, Position, ResourceRef,
    RunInputs, TargetLocator, TargetScope, UiOperation, WindowIdentity, WindowTitleMatcher,
    WorkflowDefinition, WorkflowEdge, WorkflowNode, WorkflowNodeKind, WorkflowPermissions,
};
use argusflow_runtime::{ExecutionEventSink, WorkflowEngine};
use argusflow_windows::{
    uia::{UiaBackend, UiaRuntime},
    window::WindowsApplicationSessionProvider,
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
async fn app_session_launches_then_restores_the_same_notepadpp_window() {
    let (spec, document) = create_spec("window-lifecycle");
    let provider = WindowsApplicationSessionProvider;
    let first = provider
        .acquire(&spec)
        .await
        .expect("provider should launch a unique Notepad++ window");
    let [first_window] = first.windows.as_slice() else {
        panic!("application session should contain one window");
    };
    let cleanup = WindowCleanup {
        window: *first_window,
        document,
    };

    let native = native_window(first_window.handle);
    // SAFETY: HWND 由刚完成的应用会话获取返回，只请求最小化该测试窗口。
    let _ = unsafe { ShowWindowAsync(native, SW_MINIMIZE) };
    tokio::time::sleep(Duration::from_millis(300)).await;
    // SAFETY: 测试仍拥有该窗口的生命周期，调用只读取最小化状态。
    assert!(unsafe { IsIconic(native) }.as_bool());

    let restored = provider
        .acquire(&spec)
        .await
        .expect("provider should restore the existing Notepad++ window");
    assert_eq!(restored.windows, first.windows);
    assert!(!restored.started_by_workflow);
    // SAFETY: restored HWND 已由应用会话提供器再次枚举和恢复。
    assert!(!unsafe { IsIconic(native) }.as_bool());

    drop(cleanup);
}

/// 产品完整路径必须产生 AppSession、解析资源作用域并执行真实 UIA Invoke。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an interactive Windows desktop and 64-bit English Notepad++"]
async fn workflow_application_resource_scopes_a_real_uia_action() {
    let (spec, document) = create_spec("uia-invoke");
    let runtime = Arc::new(UiaRuntime::start());
    assert!(runtime.health().is_ready(), "UIA worker should initialize");
    let backends: Vec<Arc<dyn ActionBackend>> = vec![Arc::new(UiaBackend::new(runtime))];
    let router = Arc::new(ActionRouter::new(backends));
    let applications = Arc::new(WindowsApplicationSessionProvider);
    let engine = Arc::new(WorkflowEngine::with_application_provider(
        router,
        applications,
    ));
    let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
    engine
        .start(
            application_workflow(spec.clone()),
            RunInputs::default(),
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
    let window = wait_for_test_window(spec.window_title.value(), Duration::from_secs(2))
        .expect("application resource should leave its uniquely titled window running");
    let cleanup = WindowCleanup { window, document };
    assert_eq!(
        events.last().map(|event| event.kind),
        Some(ExecutionEventKind::WorkflowCompleted)
    );
    assert!(events.iter().any(|event| {
        event.kind == ExecutionEventKind::ResourceAcquired
            && event.node_id.as_deref() == Some("application")
    }));
    assert!(events.iter().any(|event| {
        event.kind == ExecutionEventKind::BackendSelected
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

/// 构造 Start → Application → Ui → End 的真实资源数据路径。
fn application_workflow(spec: ApplicationSpec) -> WorkflowDefinition {
    WorkflowDefinition {
        schema_version: 5,
        id: Uuid::new_v4(),
        name: "Notepad++ AppSession E2E".to_owned(),
        inputs: Vec::new(),
        variables: json!({}),
        permissions: WorkflowPermissions {
            application_launch: true,
            direct_command: false,
            powershell: false,
            cmd: false,
        },
        nodes: vec![
            node("start", 0.0, WorkflowNodeKind::Start),
            node("application", 200.0, WorkflowNodeKind::Application { spec }),
            node(
                "invoke-help",
                400.0,
                WorkflowNodeKind::Ui {
                    operation: UiOperation::Click {
                        target: argusflow_core::AutomationTarget {
                            scope: TargetScope::Application {
                                resource: ResourceRef {
                                    producer_node_id: "application".to_owned(),
                                    output_name: "session".to_owned(),
                                },
                            },
                            locator: TargetLocator::Query {
                                query: argusflow_core::AqlQuery::v1(
                                    r#"first(window(name contains "Notepad++") >> menu_item(name = "?"))"#,
                                ),
                            },
                            backend_preference: BackendPreference::WindowsUia,
                        },
                    },
                },
            ),
            node("end", 600.0, WorkflowNodeKind::End),
        ],
        edges: vec![
            edge("start", "application"),
            edge("application", "invoke-help"),
            edge("invoke-help", "end"),
        ],
    }
}

/// 使用给定横坐标创建测试节点。
fn node(id: &str, x: f64, kind: WorkflowNodeKind) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        position: Position { x, y: 0.0 },
        kind,
    }
}

/// 创建线性无分支测试边。
fn edge(source: &str, target: &str) -> WorkflowEdge {
    WorkflowEdge {
        id: format!("{source}-{target}"),
        source: source.to_owned(),
        target: target.to_owned(),
        branch: None,
    }
}

/// 为每个测试创建标题唯一的 Notepad++ 文档和应用契约。
fn create_spec(scenario: &str) -> (ApplicationSpec, PathBuf) {
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
    let spec = ApplicationSpec {
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
        acquire_policy: AcquirePolicy::AttachOrStart,
        launch_timeout_ms: 10_000,
        cleanup_policy: CleanupPolicy::LeaveRunning,
        activation_policy: ActivationPolicy::BestEffort,
    };
    (spec, document)
}

/// 只读等待测试创建的唯一标题窗口，避免重新激活导致展开菜单关闭。
fn wait_for_test_window(title_fragment: &str, timeout: Duration) -> Option<WindowIdentity> {
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
fn find_test_window(title_fragment: &str) -> Option<WindowIdentity> {
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
    window: Option<WindowIdentity>,
}

/// 枚举可见标题窗口并冻结 HWND/PID，匹配成功后提前停止。
unsafe extern "system" fn enum_test_window(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: parameter 由 find_test_window 传入，同步 EnumWindows 返回前始终有效。
    let search = unsafe { &mut *(parameter.0 as *mut TestWindowSearch<'_>) };
    // SAFETY: window 由系统枚举提供，只执行只读可见性检查。
    if !unsafe { IsWindowVisible(window) }.as_bool() {
        return true.into();
    }
    // SAFETY: window 是同步 callback 提供的枚举值，只读取标题长度。
    let title_length = unsafe { GetWindowTextLengthW(window) };
    if title_length <= 0 {
        return true.into();
    }
    let mut title = vec![0_u16; title_length as usize + 1];
    // SAFETY: title buffer 可写且为结尾留有空间。
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
    search.window = Some(WindowIdentity {
        handle: window.0 as usize as u64,
        process_id,
    });
    false.into()
}

/// 真实工作流 E2E 使用的内存事件接收器。
struct ChannelSink(mpsc::UnboundedSender<ExecutionEvent>);

impl ExecutionEventSink for ChannelSink {
    fn emit(&self, event: ExecutionEvent) -> Result<(), String> {
        self.0.send(event).map_err(|error| error.to_string())
    }
}

/// 无论断言是否完成，都关闭测试创建的窗口并删除精确临时文件。
struct WindowCleanup {
    /// 测试创建的唯一 Notepad++ 窗口。
    window: WindowIdentity,
    /// 测试创建且只由本 guard 删除的临时文档。
    document: PathBuf,
}

impl Drop for WindowCleanup {
    fn drop(&mut self) {
        let native = native_window(self.window.handle);
        // SAFETY: HWND 来自本测试启动的唯一标题窗口；消息不携带外部指针。
        let _ = unsafe { PostMessageW(Some(native), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        let _ = fs::remove_file(&self.document);
    }
}

/// 把稳定整数窗口身份转换为 HWND 不透明值。
fn native_window(handle: u64) -> HWND {
    HWND(handle as usize as *mut std::ffi::c_void)
}
