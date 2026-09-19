//! 可验证的文本传输任务，复用浏览器、UIA 和剪贴板能力。
mod clipboard;
mod compiler;
mod task;
pub(crate) use compiler::register;
