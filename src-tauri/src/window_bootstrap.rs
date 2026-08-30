//! 主窗口静态启动页就绪后的原生显示控制。

use tauri::{Runtime, plugin::TauriPlugin};

/// 在每个真实主文档创建时等待静态启动页节点，并通过 Tauri 窗口命令显示窗口。
///
/// 该脚本由 WebView2 在文档创建阶段直接注入，不进入 Vite 模块图，因此不会为了显示
/// 首屏而触发 `@tauri-apps/api/window` 的依赖扫描与预转换。
const REVEAL_MAIN_WINDOW_SCRIPT: &str = r#"
(() => {
  const reveal = () => {
    if (!document.getElementById('argusflow-boot-loading')) return;
    observer.disconnect();
    const label = window.__TAURI_INTERNALS__.metadata.currentWindow.label;
    window.__TAURI_INTERNALS__
      .invoke('plugin:window|show', { label })
      .catch((error) => console.error('ArgusFlow 主窗口显示失败。', error));
  };
  const observer = new MutationObserver(reveal);
  observer.observe(document, { childList: true, subtree: true });
  reveal();
})();
"#;

/// 创建只负责注入首屏显示脚本的轻量 Tauri 插件。
pub(crate) fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("window-bootstrap")
        .js_init_script(REVEAL_MAIN_WINDOW_SCRIPT)
        .build()
}
