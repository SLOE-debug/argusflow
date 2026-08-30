fn main() {
    // Tauri 当前不会在本项目的开发构建中把 bundle 图标写入 Cargo 依赖列表。
    // 显式追踪 Windows 图标，避免只替换 ICO 时继续复用嵌有旧资源的可执行文件。
    println!("cargo:rerun-if-changed=icons/icon.ico");

    // 将 Tauri 命令清单写入应用清单，并在失败时中止构建脚本。
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "begin_runtime_initialization",
            "get_startup_status",
            "retry_startup",
            "inspect_aql",
            "validate_workflow",
            "run_workflow",
        ]));

    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("failed to prepare the ArgusFlow Tauri build: {error}");
        std::process::exit(1);
    }
}
