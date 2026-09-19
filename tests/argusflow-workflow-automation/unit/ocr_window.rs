//! 使用自有窗口验证动态 OCR 范围在恢复窗口后解析，而不是读取最小化坐标。
use super::*;
use argusflow_aql::{Bindings, compile};
use argusflow_automation::Locator;
use argusflow_capture_contracts::FrameConfig;
use argusflow_core::OperationOptions;
#[path = "../../argusflow-windows/support/window.rs"]
#[allow(dead_code)]
mod support;
use windows::Win32::UI::WindowsAndMessaging::{IsIconic, SW_MINIMIZE, ShowWindowAsync};

#[tokio::test]
#[ignore = "自有窗口最小化恢复、真实桌面采集和本机 OCR 模型回归"]
async fn minimized_ocr_scope_resolves_and_samples_after_activation() {
    let fixture = support::Fixture::create();
    let window = WindowIdentity::from_handle(fixture.window).unwrap();
    let input = InputService::new().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let services = crate::resources::OcrServices::new(
        root.join(".deps"),
        FrameConfig {
            width: 3840,
            height: 2160,
            ..Default::default()
        },
    );
    let operation = Operation::new(OperationOptions::new(Duration::from_secs(120)).unwrap());
    let result = async {
        let binding = OcrWindow {
            session: services.acquire(&operation).await?,
            window: window.clone(),
            input: input.clone(),
        };
        binding.ready(&operation).await?;
        unsafe { ShowWindowAsync(fixture.hwnd(), SW_MINIMIZE) }.ok()?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(unsafe { IsIconic(fixture.hwnd()) }.as_bool());
        let source = binding.resolve(&operation).await?;
        window.require_foreground()?;
        assert!(!unsafe { IsIconic(fixture.hwnd()) }.as_bool());
        // 再次最小化，单独验证底层 OCR 来源也在采样前激活。
        unsafe { ShowWindowAsync(fixture.hwnd(), SW_MINIMIZE) }.ok()?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let query = compile("text(text contains \"initial\")")?;
        let locator = Locator::bind(source, &query, &Bindings::new())?;
        let found = locator.find_all_with_operation(&operation).await?;
        assert!(!found.is_empty(), "应识别自有窗口编辑区正文");
        window.require_foreground()?;
        Ok::<_, Box<dyn std::error::Error>>(())
    }
    .await;
    input.shutdown(OperationOptions::default()).await.unwrap();
    services
        .shutdown(&Operation::new(OperationOptions::default()))
        .await
        .unwrap();
    result.unwrap();
}
