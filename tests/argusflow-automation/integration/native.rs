#![cfg(windows)]
//! 只操作本进程测试窗口，验证定位、点击及光标插入。
#[path = "../../argusflow-windows/support/window.rs"]
mod support;
use argusflow_aql::{Bindings, compile};
use argusflow_automation::{Locator, QuerySource};
use argusflow_core::{Key, OperationOptions};
use argusflow_windows::*;
use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "opens an isolated test window and temporarily uses mouse/keyboard"]
async fn aql_native_click_and_insert() {
    let _dpi = support::DpiContext::physical();
    let fixture = support::Fixture::create();
    assert_eq!(
        fixture.message(103, windows::Win32::UI::WindowsAndMessaging::BM_GETCHECK),
        0
    );
    let window = WindowLocator {
        process_id: Some(std::process::id()),
        title: Some("ArgusFlow Native Acceptance".into()),
        ..Default::default()
    }
    .find_unique()
    .unwrap()
    .identity();
    let runtime = UiaRuntime::start(UiaConfig::default(), OperationOptions::default())
        .await
        .unwrap();
    let input = InputService::new().unwrap();
    let operation = argusflow_core::Operation::new(OperationOptions::default());
    let sequence = input.sequence(&operation).unwrap();
    assert!(
        matches!(input.sequence(&operation), Err(error) if error.kind() == argusflow_core::FailureKind::Busy)
    );
    drop(sequence);
    let source = QuerySource::Uia {
        runtime: runtime.clone(),
        window: window.clone(),
        input: input.clone(),
    };
    let result = async {
        unsafe {
            let _ = SetForegroundWindow(fixture.hwnd());
        }
        window.require_foreground()?;
        let attributes = compile("window() >> element(checked = true or value = \"initial\")")?
            .bind(&Bindings::new())?;
        let matches = runtime
            .query_aql(
                window.clone(),
                attributes,
                &argusflow_core::Operation::new(OperationOptions::default()),
            )
            .await?;
        assert_eq!(matches.len(), 1);
        matches[0].handle().release();
        assert_eq!(
            runtime
                .aql_click_point(
                    matches[0].handle(),
                    &argusflow_core::Operation::new(OperationOptions::default())
                )
                .await
                .unwrap_err()
                .kind(),
            argusflow_core::FailureKind::StaleHandle
        );
        let button = Locator::bind(
            source.clone(),
            &compile("button(name = \"Invoke target\", enabled = true)").unwrap(),
            &Bindings::new(),
        )
        .unwrap();
        if let argusflow_automation::LocatedElement::Uia(found) =
            button.find_unique(OperationOptions::default()).await?
        {
            let mut rect = windows::Win32::Foundation::RECT::default();
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetWindowRect(
                    fixture.child(101),
                    &mut rect,
                )?;
            }
            assert_eq!(
                found.snapshot().bounds,
                [rect.left, rect.top, rect.right, rect.bottom]
            );
            found.handle().release();
        }
        button.click(OperationOptions::default()).await?;
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while fixture.text(104) != "Invoke received" && std::time::Instant::now() < until {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let mut cursor = windows::Win32::Foundation::POINT::default();
        let mut rect = windows::Win32::Foundation::RECT::default();
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor)?;
            windows::Win32::UI::WindowsAndMessaging::GetWindowRect(fixture.child(101), &mut rect)?;
        }
        assert!(
            cursor.x >= rect.left
                && cursor.x < rect.right
                && cursor.y >= rect.top
                && cursor.y < rect.bottom
        );
        assert_eq!(fixture.text(104), "Invoke received");
        let field = runtime
            .find_unique(
                Query {
                    window: window.clone(),
                    predicate: Predicate::ControlType(ControlType::Edit),
                    scope: SearchScope::Descendants,
                },
                OperationOptions::default(),
            )
            .await?;
        runtime
            .perform(
                &field,
                UiaAction::SetValue("已有".into()),
                OperationOptions::default(),
            )
            .await?;
        runtime
            .perform(&field, UiaAction::Focus, OperationOptions::default())
            .await?;
        input
            .perform(
                window.clone(),
                InputAction::Chord(vec![Key::End]),
                OperationOptions::default(),
            )
            .await?;
        input
            .perform(
                window.clone(),
                InputAction::Chord(vec![Key::Shift, Key::Home]),
                OperationOptions::default(),
            )
            .await?;
        field.release();
        let editor =
            Locator::bind(source, &compile("textbox()").unwrap(), &Bindings::new()).unwrap();
        editor
            .type_text("追加", OperationOptions::default())
            .await?;
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        assert_eq!(fixture.text(102), "已有追加");
        Ok::<_, Box<dyn std::error::Error>>(())
    }
    .await;
    input.shutdown(OperationOptions::default()).await.unwrap();
    runtime.shutdown(OperationOptions::default()).await.unwrap();
    result.unwrap();
}
