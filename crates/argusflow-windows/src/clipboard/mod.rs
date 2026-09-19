//! 有界只读剪贴板服务，不打开 UI，不改写内容，不持有剪贴板快照历史。
mod read;
mod service;
pub use service::ClipboardReader;
