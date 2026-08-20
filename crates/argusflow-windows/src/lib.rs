//! Windows 原生自动化、输入注入、窗口管理与画面捕获能力。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

/// Windows 桌面与窗口画面捕获能力。
pub mod capture;
/// Windows 输入事件注入能力。
pub mod input;
/// Windows UI Automation 能力。
pub mod uia;
/// Win32 窗口枚举与管理能力。
pub mod window;
