//! 桌面程序入口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    argusflow_desktop::run();
}
