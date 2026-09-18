use super::*;

#[test]
fn ocr_path_belongs_to_application_not_workflow() {
    assert_eq!(
        beside_executable(Path::new("C:/Apps/ArgusFlow/argusflow.exe")).unwrap(),
        PathBuf::from("C:/Apps/ArgusFlow/ocr")
    );
}

#[tokio::test]
#[ignore = "加载 Tauri 构建产物旁的内置 OCR 模型与 DLL，验证实际打包资源"]
async fn bundled_ocr_models_load() {
    let executable = std::env::current_exe().unwrap();
    let app = executable
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("argusflow.exe");
    let root = beside_executable(&app).unwrap();
    assert!(root.join("runtime/cpu/onnxruntime.dll").is_file());
    let engine = argusflow_vision::OcrEngine::load_with_options(
        argusflow_vision::OcrConfig::new(root),
        argusflow_core::OperationOptions::new(std::time::Duration::from_secs(60)).unwrap(),
    )
    .await
    .unwrap();
    engine
        .shutdown(argusflow_core::OperationOptions::default())
        .await
        .unwrap();
}
