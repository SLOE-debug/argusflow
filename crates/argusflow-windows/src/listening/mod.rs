//! 被动输入监听，独立于 InputService 注入服务。
mod callbacks;
mod scope;
mod service;
pub use service::{InputListener, ListenerState, gesture_settings, qpc, qpc_frequency};
