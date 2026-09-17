//! 开发启动计时：区分原生窗口创建与前端首次显示的等待。
use std::time::Instant;
use tauri::{Manager, State};

/// 从桌面装配开始计时，不包含 Cargo 编译和 Vite 启动。
pub(crate) struct StartupTiming(Instant);

impl Default for StartupTiming {
    fn default() -> Self {
        Self(Instant::now())
    }
}

impl StartupTiming {
    fn report(&self, stage: &str) {
        if cfg!(debug_assertions) {
            eprintln!("[startup] {stage}: {} ms", self.0.elapsed().as_millis());
        }
    }
}

/// 原生窗口与 WebView 构造完成；此时前端可能仍在加载模块。
pub(crate) fn native_ready(app: &tauri::App) {
    app.state::<StartupTiming>().report("native-ready");
}

/// 前端完成首次渲染并显示窗口后报告，不等待工作区数据加载。
#[tauri::command]
pub(crate) fn frontend_ready(timing: State<'_, StartupTiming>) {
    timing.report("window-shown");
}
