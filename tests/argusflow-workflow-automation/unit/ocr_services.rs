use super::*;
use argusflow_core::OperationOptions;

#[tokio::test]
#[ignore = "真实加载本机 OCR 模型并启动桌面采集，验证多绑定共享生命周期"]
async fn shared_native_ocr_session() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let dependencies = root.join(".deps");
    let services = OcrServices::new(dependencies, FrameConfig::default());
    let op = Operation::new(OperationOptions::new(Duration::from_secs(120)).unwrap());
    let first = services.acquire(&op).await.unwrap();
    let second = services.acquire(&op).await.unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    drop(first);
    assert!(second.frames.sources().is_ok());
    services
        .shutdown(&Operation::new(OperationOptions::default()))
        .await
        .unwrap();
    assert!(services.state.lock().await.active.is_none());
    services
        .shutdown(&Operation::new(OperationOptions::default()))
        .await
        .unwrap();
}

#[tokio::test]
async fn cancelled_waiter_does_not_initialize_or_hold_service_lock() {
    let services = OcrServices::default();
    let held = services.state.lock().await;
    let op = Operation::new(OperationOptions::default());
    op.cancel();
    let result = services.acquire(&op).await;
    assert!(matches!(
        result,
        Err(RunError {
            kind: ErrorKind::Cancelled,
            ..
        })
    ));
    assert!(held.active.is_none());
    drop(held);
    services
        .shutdown(&Operation::new(OperationOptions::default()))
        .await
        .unwrap();
}

#[tokio::test]
async fn waiting_for_initialization_respects_total_deadline() {
    let services = OcrServices::default();
    let held = services.state.lock().await;
    let op = Operation::new(OperationOptions::new(Duration::from_millis(10)).unwrap());
    let result = services.acquire(&op).await;
    assert!(matches!(
        result,
        Err(RunError {
            kind: ErrorKind::Timeout,
            ..
        })
    ));
    assert!(held.active.is_none());
}

#[tokio::test]
async fn closed_service_rejects_requests_and_shutdown_is_idempotent() {
    let services = OcrServices::default();
    for _ in 0..2 {
        services
            .shutdown(&Operation::new(OperationOptions::default()))
            .await
            .unwrap();
    }
    let result = services
        .acquire(&Operation::new(OperationOptions::default()))
        .await;
    assert!(matches!(
        result,
        Err(RunError {
            kind: ErrorKind::Unavailable,
            ..
        })
    ));
}
