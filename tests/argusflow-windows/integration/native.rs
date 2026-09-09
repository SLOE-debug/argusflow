#![cfg(windows)]
//! 用户显式运行的真实 UIA / SendInput 验收，不操作日常应用。
#[path = "../support/window.rs"]
mod support;
use argusflow_core::{ClickCount, FailureKind, Key, MouseButton, ScreenPoint, ScrollAxis};
use argusflow_windows::*;
use std::{sync::atomic::Ordering, time::Duration};
use windows::Win32::{
    Foundation::POINT,
    Graphics::Gdi::ClientToScreen,
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_SHIFT},
        WindowsAndMessaging::*,
    },
};

fn options() -> OperationOptions {
    OperationOptions::default()
}
fn query(window: &WindowIdentity, predicate: Predicate) -> Query {
    Query {
        window: window.clone(),
        predicate,
        scope: SearchScope::Descendants,
    }
}
async fn find(runtime: &UiaRuntime, window: &WindowIdentity, name: &str) -> ElementHandle {
    runtime
        .find_unique(query(window, Predicate::Name(name.into())), options())
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "opens and operates an isolated test window, temporarily uses mouse/keyboard"]
async fn uia_patterns_and_real_input() {
    let _dpi = support::DpiContext::physical();
    let fixture = support::Fixture::create();
    let window = WindowLocator {
        process_id: Some(std::process::id()),
        title: Some("ArgusFlow Native Acceptance".into()),
        ..Default::default()
    }
    .find_unique()
    .unwrap()
    .identity();
    let runtime = UiaRuntime::start(UiaConfig::default(), options())
        .await
        .unwrap();
    let input = InputService::new().unwrap();
    // 普通操作错误也先执行显式 shutdown，再报告失败；panic 时由 Drop 清理。
    let result = exercise(&fixture, &window, &runtime, &input).await;
    input.shutdown(options()).await.unwrap();
    runtime.shutdown(options()).await.unwrap();
    result.unwrap();
}

async fn exercise(
    fixture: &support::Fixture,
    window: &WindowIdentity,
    runtime: &UiaRuntime,
    input: &InputService,
) -> Result<(), Box<dyn std::error::Error>> {
    let button = find(runtime, window, "Invoke target").await;
    let snapshot = runtime.read(&button, options()).await?;
    assert_eq!(snapshot.control_type, ControlType::Button as i32);
    assert!(!snapshot.automation_id.is_empty());
    let by_id = runtime
        .find_unique(
            query(
                window,
                Predicate::All(vec![
                    Predicate::AutomationId(snapshot.automation_id),
                    Predicate::ClassName(snapshot.class_name),
                    Predicate::ControlType(ControlType::Button),
                ]),
            ),
            options(),
        )
        .await?;
    by_id.release();
    assert_eq!(
        runtime
            .find_unique(query(window, Predicate::Any), options())
            .await
            .unwrap_err()
            .kind(),
        FailureKind::Ambiguous
    );
    runtime
        .perform(&button, UiaAction::Invoke, options())
        .await?;
    wait(|| fixture.text(104) == "Invoke received").await?;
    println!("PASS window identity, unique/ambiguous queries, properties and Invoke");
    let edit = runtime
        .find_unique(
            query(window, Predicate::ControlType(ControlType::Edit)),
            options(),
        )
        .await?;
    runtime
        .perform(&edit, UiaAction::SetValue("UIA 中文 123".into()), options())
        .await?;
    assert_eq!(fixture.text(102), "UIA 中文 123");
    let toggle = find(runtime, window, "Toggle target").await;
    assert_eq!(fixture.message(103, BM_GETCHECK), 0);
    runtime
        .perform(&toggle, UiaAction::Toggle, options())
        .await?;
    wait(|| fixture.message(103, BM_GETCHECK) == 1).await?;
    runtime
        .perform(&toggle, UiaAction::Toggle, options())
        .await?;
    wait(|| fixture.message(103, BM_GETCHECK) == 0).await?;
    let item = find(runtime, window, "Item 02").await;
    runtime
        .perform(
            &item,
            UiaAction::Selection(SelectionAction::Select),
            options(),
        )
        .await?;
    assert_eq!(fixture.message(105, LB_GETCURSEL), 2);
    println!("PASS SetValue, Toggle and Selection");
    let parent = find(runtime, window, "Parent").await;
    runtime
        .perform(&parent, UiaAction::Expand, options())
        .await?;
    let child = find(runtime, window, "Child").await;
    assert!(!runtime.read(&child, options()).await?.offscreen);
    runtime
        .perform(&parent, UiaAction::Collapse, options())
        .await?;
    let list = runtime
        .find_unique(
            query(window, Predicate::ControlType(ControlType::List)),
            options(),
        )
        .await?;
    runtime
        .perform(
            &list,
            UiaAction::Scroll {
                axis: ScrollAxis::Vertical,
                amount: ScrollAmount::LargeIncrement,
            },
            options(),
        )
        .await?;
    assert!(fixture.message(105, LB_GETTOPINDEX) > 0);
    let last = find(runtime, window, "Item 39").await;
    runtime
        .perform(&last, UiaAction::ScrollIntoView, options())
        .await?;
    assert!(!runtime.read(&last, options()).await?.offscreen);
    let error = runtime
        .perform(
            &button,
            UiaAction::SetValue("unsupported".into()),
            options(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Unsupported);
    println!("PASS Expand/Collapse, control scrolling, ScrollIntoView and unsupported Pattern");
    // 测试宿主显式激活自己，库的输入服务仍必须独立校验前台。
    unsafe {
        let _ = SetForegroundWindow(fixture.hwnd());
    }
    window.require_foreground()?;
    runtime.perform(&edit, UiaAction::Focus, options()).await?;
    wait(|| {
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetGUIThreadInfo(GetWindowThreadProcessId(fixture.hwnd(), None), &mut info) }
            .is_ok()
            && info.hwndFocus == fixture.child(102)
    })
    .await?;
    // 经典 Win32 EDIT 没有内置 Ctrl+A；使用其标准 Home、Shift+End 选择当前行。
    input
        .perform(
            window.clone(),
            InputAction::Chord(vec![Key::Home]),
            options(),
        )
        .await?;
    input
        .perform(
            window.clone(),
            InputAction::Chord(vec![Key::Shift, Key::End]),
            options(),
        )
        .await?;
    input
        .perform(
            window.clone(),
            InputAction::Text("真实输入 456".into()),
            options(),
        )
        .await?;
    if let Err(error) = wait(|| fixture.text(102) == "真实输入 456").await {
        eprintln!("Observed edit text after input: {:?}", fixture.text(102));
        return Err(error);
    }
    assert!(unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) } >= 0);
    assert!(unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) } >= 0);
    println!("PASS Focus, SendInput Home/Shift+End, Unicode text and released modifiers");
    let mut point = POINT { x: 500, y: 430 };
    unsafe { ClientToScreen(fixture.hwnd(), &mut point) }.ok()?;
    let point = ScreenPoint {
        x: point.x,
        y: point.y,
    };
    input
        .perform(window.clone(), InputAction::Move(point), options())
        .await?;
    for (button, count) in [
        (MouseButton::Left, ClickCount::Single),
        (MouseButton::Right, ClickCount::Single),
        (MouseButton::Left, ClickCount::Double),
    ] {
        input
            .perform(
                window.clone(),
                InputAction::Click {
                    point,
                    button,
                    count,
                },
                options(),
            )
            .await?;
    }
    for axis in [ScrollAxis::Vertical, ScrollAxis::Horizontal] {
        input
            .perform(
                window.clone(),
                InputAction::Wheel {
                    point,
                    axis,
                    steps: 1,
                },
                options(),
            )
            .await?;
    }
    wait(|| {
        support::LEFT.load(Ordering::SeqCst) > 0
            && support::RIGHT.load(Ordering::SeqCst) > 0
            && support::DOUBLE.load(Ordering::SeqCst) > 0
            && support::VERTICAL.load(Ordering::SeqCst) > 0
            && support::HORIZONTAL.load(Ordering::SeqCst) > 0
    })
    .await?;
    println!("PASS mouse movement, left/right/double click and both wheel axes");
    unsafe {
        let _ = ShowWindow(fixture.hwnd(), SW_MINIMIZE);
    }
    wait(|| window.require_foreground().is_err()).await?;
    let error = input
        .perform(
            window.clone(),
            InputAction::Text("must not type".into()),
            options(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.stage(), "foreground");
    unsafe {
        let _ = ShowWindow(fixture.hwnd(), SW_RESTORE);
        let _ = SetForegroundWindow(fixture.hwnd());
    }
    window.require_foreground()?;
    let error = input
        .perform(
            window.clone(),
            InputAction::Move(ScreenPoint {
                x: -100_000,
                y: -100_000,
            }),
            options(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::InvalidInput);
    let overlay = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            windows::core::w!("BUTTON"),
            windows::core::w!("ArgusFlow occlusion fixture"),
            WS_POPUP | WS_VISIBLE,
            point.x - 30,
            point.y - 30,
            60,
            60,
            None,
            None,
            None,
            None,
        )
    }?;
    let blocked = input
        .perform(
            window.clone(),
            InputAction::Click {
                point,
                button: MouseButton::Left,
                count: ClickCount::Single,
            },
            options(),
        )
        .await;
    unsafe { DestroyWindow(overlay) }?;
    assert_eq!(blocked.unwrap_err().kind(), FailureKind::InvalidInput);
    println!("PASS foreground, out-of-window and occlusion guards");
    println!(
        "DISPLAY monitors={} virtual=({}, {}, {}, {}) dpi={}",
        unsafe { GetSystemMetrics(SM_CMONITORS) },
        unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
        unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(fixture.hwnd()) }
    );
    button.release();
    assert_eq!(
        runtime.read(&button, options()).await.unwrap_err().kind(),
        FailureKind::StaleHandle
    );
    Ok(())
}
async fn wait(condition: impl Fn() -> bool) -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    while !condition() {
        if start.elapsed() > Duration::from_secs(3) {
            return Err("expected native state was not observed".into());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Ok(())
}
