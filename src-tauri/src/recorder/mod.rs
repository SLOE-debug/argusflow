//! 桌面录制装配、子进程与证据适配。
mod child;
mod commands;
mod context;
mod evidence;
mod manager;
mod messages;
mod pipes;
mod reader;
mod supervisor;
mod video;
mod watchdog;
pub use child::run_child;
pub use commands::*;
pub use manager::RecorderManager;
pub use video::*;
