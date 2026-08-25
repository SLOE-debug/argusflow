//! 真实 Notepad++ 进程的启动、最小化恢复与窗口身份验收。

use std::{env, fs, path::PathBuf, sync::Arc, time::Duration};

use argusflow_agent::{
    AccessibilityContext, ActionBackend, ActionRouter, ExecutionContext, StaticExecutionContext,
};
use argusflow_core::{
    ApplicationTarget, AqlQuery, AutomationAction, AutomationTarget, BackendKind,
    BackendPreference, TargetLocator, WindowTitleMatcher,
};
use argusflow_windows::{
    uia::{UiaBackend, UiaRuntime},
    window::{ResolvedWindow, WindowService},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{IsIconic, PostMessageW, SW_MINIMIZE, ShowWindowAsync, WM_CLOSE},
};

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

/// 应用作用域必须在无前台窗口上下文时启动 Notepad++ 并继续执行真实 UIA Invoke。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an interactive Windows desktop and ARGUSFLOW_NOTEPADPP_EXE"]
async fn application_query_launches_notepadpp_before_uia_invoke() {
    let (application, document) = create_target("uia-invoke");
    let runtime = Arc::new(UiaRuntime::start());
    assert!(runtime.health().is_ready(), "UIA worker should initialize");
    let context = ExecutionContext {
        accessibility: AccessibilityContext { ready: true },
        ..ExecutionContext::default()
    };
    let context_provider = Arc::new(StaticExecutionContext::new(context.clone()));
    let backends: Vec<Arc<dyn ActionBackend>> = vec![Arc::new(UiaBackend::new(runtime))];
    let router = ActionRouter::with_context_provider(backends, context_provider);
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

    let action_result = router
        .prepare(&action, &context)
        .expect("application query should produce a ready UIA plan")
        .execute()
        .await;
    let window = WindowService
        .resolve_application(application)
        .await
        .expect("application query should leave a resolvable Notepad++ window");
    let cleanup = WindowCleanup { window, document };
    let outcome = action_result.expect("application query should invoke the first app menu");
    assert_eq!(outcome.backend, BackendKind::WindowsUia);

    drop(cleanup);
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
