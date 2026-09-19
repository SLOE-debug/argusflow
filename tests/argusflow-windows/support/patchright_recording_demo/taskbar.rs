//! 通过任务栏的实际 UIA 应用按钮切换窗口，不绕过 Windows 前台限制。
use super::Result;
use argusflow_windows::{
    OperationOptions, Predicate, Query, SearchScope, UiaRuntime, WindowIdentity, WindowLocator,
};
use std::time::Duration;

pub async fn activate(
    runtime: &UiaRuntime,
    input: &argusflow_windows::InputService,
    target: &WindowIdentity,
    app_id: &str,
) -> Result<()> {
    if target.require_foreground().is_ok() {
        return Ok(());
    }
    let taskbar = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match (WindowLocator {
                class_name: Some("Shell_TrayWnd".into()),
                ..Default::default()
            })
            .find_unique()
            {
                Ok(window) => return Ok::<_, Box<dyn std::error::Error>>(window.identity()),
                Err(error) if error.kind() == argusflow_core::FailureKind::NotFound => {
                    tokio::time::sleep(Duration::from_millis(100)).await
                }
                Err(error) => return Err(error.into()),
            }
        }
    })
    .await??;
    let button = runtime
        .find_unique(
            Query {
                window: taskbar.clone(),
                predicate: Predicate::AutomationId(app_id.into()),
                scope: SearchScope::Descendants,
            },
            OperationOptions::default(),
        )
        .await?;
    let point = runtime
        .aql_click_point(
            &button,
            &argusflow_core::Operation::new(OperationOptions::default()),
        )
        .await;
    button.release();
    input
        .perform(
            taskbar,
            argusflow_windows::InputAction::FocusClick(point?),
            OperationOptions::default(),
        )
        .await?;
    tokio::time::timeout(Duration::from_secs(3), async {
        while target.require_foreground().is_err() {
            target.validate()?;
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })
    .await??;
    Ok(())
}
