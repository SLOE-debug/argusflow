//! 用户手动运行；操作范围仅限标题完全匹配的测试窗口。
#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use argusflow_windows::{
        OperationOptions, Predicate, Query, SearchScope, UiaAction, UiaConfig, UiaRuntime,
        WindowLocator,
    };
    let window = WindowLocator {
        title: Some("ArgusFlow UIA Test".into()),
        ..Default::default()
    }
    .find_unique()?
    .identity();
    let runtime = UiaRuntime::start(UiaConfig::default(), OperationOptions::default()).await?;
    let result = async {
        let button = runtime
            .find_unique(
                Query {
                    window,
                    predicate: Predicate::Name("Invoke me".into()),
                    scope: SearchScope::Descendants,
                },
                OperationOptions::default(),
            )
            .await?;
        println!(
            "{:?}",
            runtime.read(&button, OperationOptions::default()).await?
        );
        runtime
            .perform(&button, UiaAction::Invoke, OperationOptions::default())
            .await?;
        button.release();
        Ok::<_, argusflow_windows::WindowsError>(())
    }
    .await;
    runtime.shutdown(OperationOptions::default()).await?;
    result?;
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("UIA requires Windows.");
}
