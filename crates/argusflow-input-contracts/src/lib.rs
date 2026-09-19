//! 被动输入来源契约，不包含注入、平台句柄所有权或录制规则。
mod event;
pub use event::*;
mod clipboard;
pub use clipboard::{ClipboardContent, ClipboardObservation};
