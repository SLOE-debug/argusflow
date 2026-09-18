//! 在独立实例中验证 demo 依赖的真实菜单标识和新建/关闭行为。
use argusflow_core::{Operation, OperationOptions};
use argusflow_windows::{
    Application, ApplicationOptions, Predicate, Query, SearchScope, UiaAction, UiaRuntime,
    WindowIdentity,
};

async fn perform(
    runtime: &UiaRuntime,
    window: &WindowIdentity,
    predicate: Predicate,
    action: UiaAction,
) -> Result<(), argusflow_windows::WindowsError> {
    let operation = Operation::new(OperationOptions::default());
    loop {
        operation.check("wait_notepad_menu")?;
        let handles = runtime
            .find_all(
                Query {
                    window: window.clone(),
                    predicate: predicate.clone(),
                    scope: SearchScope::Descendants,
                },
                OperationOptions::default(),
            )
            .await?;
        if handles.len() == 1 {
            return runtime
                .perform(&handles[0], action, OperationOptions::default())
                .await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test]
#[ignore = "启动独立 Notepad++，通过真实 UIA 菜单新建并关闭空白标签"]
async fn notepad_uia_new_and_close() {
    let op = Operation::new(OperationOptions::default());
    let mut options = ApplicationOptions::new(r"C:\Program Files\Notepad++\notepad++.exe");
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/argusflow-workflow-automation/fixtures/notepad-ocr-while.workflow.json");
    options.arguments = vec![
        "-multiInst".into(),
        "-nosession".into(),
        "-noPlugin".into(),
        fixture.to_string_lossy().into_owned(),
    ];
    let app = Application::launch(options, &op).unwrap();
    let runtime = UiaRuntime::start(Default::default(), OperationOptions::default())
        .await
        .unwrap();
    let result = async {
        let window = app.wait_window(None, Some("Notepad++".into()), &op).await?;
        for (name, expected) in [("新建(N)", true), ("关闭(C)", false)] {
            perform(
                &runtime,
                &window,
                Predicate::Name("文件(F)".into()),
                UiaAction::Expand,
            )
            .await?;
            perform(
                &runtime,
                &window,
                Predicate::Name(name.into()),
                UiaAction::Invoke,
            )
            .await?;
            loop {
                op.check("wait_notepad_tab")?;
                let tabs = runtime
                    .find_all(
                        Query {
                            window: window.clone(),
                            predicate: Predicate::Name("新文件 1".into()),
                            scope: SearchScope::Descendants,
                        },
                        OperationOptions::default(),
                    )
                    .await?;
                if (!tabs.is_empty()) == expected {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
        Ok::<_, argusflow_windows::WindowsError>(())
    }
    .await;
    let shutdown = runtime.shutdown(OperationOptions::default()).await;
    let cleanup = app
        .shutdown(&Operation::new(OperationOptions::default()))
        .await;
    shutdown.unwrap();
    cleanup.unwrap();
    result.unwrap();
}

/// 复现 workflow 的物理点击路径；不能用 InvokePattern 代替这个验收。
#[tokio::test]
#[ignore = "独立 Notepad++ 菜单展开后的真实鼠标点击诊断"]
async fn notepad_menu_physical_click_diagnostic() {
    use argusflow_core::{ClickCount, MouseButton, ScreenPoint};
    use argusflow_windows::{InputAction, InputService};
    let op = Operation::new(OperationOptions::default());
    let mut options = ApplicationOptions::new(r"C:\Program Files\Notepad++\notepad++.exe");
    options.arguments = vec![
        "-multiInst".into(),
        "-nosession".into(),
        "-noPlugin".into(),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/argusflow-workflow-automation/fixtures/notepad-ocr-while.workflow.json")
            .to_string_lossy()
            .into_owned(),
    ];
    let app = Application::launch(options, &op).unwrap();
    let runtime = UiaRuntime::start(Default::default(), OperationOptions::default())
        .await
        .unwrap();
    let input = InputService::new().unwrap();
    let result = async {
        let window = app.wait_window(None, Some("Notepad++".into()), &op).await?;
        perform(
            &runtime,
            &window,
            Predicate::Name("文件(F)".into()),
            UiaAction::Expand,
        )
        .await?;
        println!(
            "foreground={:?}; surfaces={:?}",
            window.require_foreground(),
            window.physical_surfaces()
        );
        let all = runtime
            .find_all(
                Query {
                    window: window.clone(),
                    predicate: Predicate::Any,
                    scope: SearchScope::Descendants,
                },
                OperationOptions::default(),
            )
            .await?;
        for handle in all {
            let info = runtime.read(&handle, OperationOptions::default()).await?;
            if info.control_type == 50011 {
                println!("MENU {:?}", info.name);
            }
        }
        let controls = loop {
            op.check("wait_new_menu")?;
            let controls = runtime
                .find_all(
                    Query {
                        window: window.clone(),
                        predicate: Predicate::Name("新建(N)".into()),
                        scope: SearchScope::Descendants,
                    },
                    OperationOptions::default(),
                )
                .await?;
            if controls.len() == 1 {
                break controls;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        };
        let snapshot = runtime
            .read(&controls[0], OperationOptions::default())
            .await?;
        println!("main_window={window:?}; new_menu_item={snapshot:?}");
        let [left, top, right, bottom] = snapshot.bounds;
        let click = input
            .perform(
                window.clone(),
                InputAction::Click {
                    point: ScreenPoint {
                        x: (left + right) / 2,
                        y: (top + bottom) / 2,
                    },
                    button: MouseButton::Left,
                    count: ClickCount::Single,
                },
                OperationOptions::default(),
            )
            .await;
        println!("PHYSICAL_CLICK_RESULT={click:#?}");
        click?;
        loop {
            op.check("wait_new_tab_after_physical_click")?;
            let tabs = runtime
                .find_all(
                    Query {
                        window: window.clone(),
                        predicate: Predicate::Name("新文件 1".into()),
                        scope: SearchScope::Descendants,
                    },
                    OperationOptions::default(),
                )
                .await?;
            if tabs.len() == 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        println!("PASS new tab observed after physical click");
        Ok::<_, argusflow_windows::WindowsError>(())
    }
    .await;
    input.shutdown(OperationOptions::default()).await.unwrap();
    runtime.shutdown(OperationOptions::default()).await.unwrap();
    app.shutdown(&Operation::new(OperationOptions::default()))
        .await
        .unwrap();
    result.unwrap();
}

#[path = "notepad_save.rs"]
mod save;
