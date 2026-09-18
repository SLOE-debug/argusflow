//! 独立实例验证新建文档另存为对话框的语义控件。
use super::*;
#[tokio::test]
#[ignore = "启动独立 Notepad++ 验证新建和保存对话框，不修改用户文件"]
async fn new_document_save_dialog() {
    let op = Operation::new(OperationOptions::default());
    let mut options = ApplicationOptions::new(r"C:\Program Files\Notepad++\notepad++.exe");
    options.arguments = vec!["-multiInst".into(), "-nosession".into(), "-noPlugin".into()];
    let app = Application::launch(options, &op).unwrap();
    let runtime = UiaRuntime::start(Default::default(), OperationOptions::default())
        .await
        .unwrap();
    let result = async {
        let window = app.wait_window(None, Some("Notepad++".into()), &op).await?;
        for item in ["新建(N)", "另存为(A)..."] {
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
                Predicate::Name(item.into()),
                UiaAction::Invoke,
            )
            .await?;
        }
        let dialog = app
            .wait_window(Some("另存为".into()), Some("#32770".into()), &op)
            .await?;
        for name in ["文件名:", "保存(S)"] {
            let found = runtime
                .find_all(
                    Query {
                        window: dialog.clone(),
                        predicate: Predicate::Name(name.into()),
                        scope: SearchScope::Descendants,
                    },
                    OperationOptions::default(),
                )
                .await?;
            assert!(!found.is_empty(), "缺少控件 {name}");
        }
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("new-document.txt");
        std::fs::write(&output, "reserved").unwrap();
        perform(
            &runtime,
            &dialog,
            Predicate::All(vec![
                Predicate::Name("文件名:".into()),
                Predicate::ControlType(argusflow_windows::ControlType::Edit),
            ]),
            UiaAction::SetValue(output.to_string_lossy().into_owned()),
        )
        .await?;
        perform(
            &runtime,
            &dialog,
            Predicate::Name("保存(S)".into()),
            UiaAction::Invoke,
        )
        .await?;
        let confirm = app
            .wait_window(Some("确认另存为".into()), Some("#32770".into()), &op)
            .await?;
        perform(
            &runtime,
            &confirm,
            Predicate::Name("是(Y)".into()),
            UiaAction::Invoke,
        )
        .await?;
        loop {
            op.check("verify_new_saved")?;
            if std::fs::read(&output).unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
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
