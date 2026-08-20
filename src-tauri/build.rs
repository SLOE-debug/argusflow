fn main() {
    // 将 Tauri 命令清单写入应用清单，并在失败时中止构建脚本。
    let attributes = tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&["validate_workflow", "run_workflow"]),
    );

    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("failed to prepare the ArgusFlow Tauri build: {error}");
        std::process::exit(1);
    }
}
