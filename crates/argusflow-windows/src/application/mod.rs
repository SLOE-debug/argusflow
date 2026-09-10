//! 自建应用进程树与窗口生命周期，不包含 workflow 逻辑。
mod command_line;
mod handle;
mod job;
mod native;
mod process;
pub use process::{Application, ApplicationOptions};
