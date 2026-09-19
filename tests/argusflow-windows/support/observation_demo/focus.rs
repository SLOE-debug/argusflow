//! 示范端借用系统任务栏切换真实前台窗口。
use super::Result;
use argusflow_core::OperationOptions;
use argusflow_windows::{
    InputService, Predicate, Query, SearchScope, UiaRuntime, WindowIdentity, WindowLocator,
};
pub async fn chrome(
    runtime: &UiaRuntime,
    input: &InputService,
    target: &WindowIdentity,
) -> Result<()> {
    if target.require_foreground().is_ok() {
        return Ok(());
    }
    let taskbar = WindowLocator {
        class_name: Some("Shell_TrayWnd".into()),
        ..Default::default()
    }
    .find_unique()?
    .identity();
    let elements = runtime
        .find_all(
            Query {
                window: taskbar,
                predicate: Predicate::Any,
                scope: SearchScope::Descendants,
            },
            OperationOptions::default(),
        )
        .await?;
    let mut ids = vec![];
    for element in elements {
        let snapshot = runtime.read(&element, OperationOptions::default()).await?;
        element.release();
        // 本 demo 的独立 chrome-profile 对应单独任务栏分组，不点击用户默认 Chrome 分组。
        if snapshot.name.contains("Google Chrome")
            && snapshot.automation_id.ends_with(".chromeprofile.Default")
        {
            ids.push(snapshot.automation_id);
        }
    }
    let [id] = ids.as_slice() else {
        return Err(format!("Chrome任务栏按钮不唯一：{ids:?}").into());
    };
    super::taskbar::activate(runtime, input, target, id).await
}
