//! 独立的真实输入能力，不占用 UIA apartment。
mod inject;
mod keyboard;
mod service;
mod submission;
pub use service::{InputAction, InputService, InputState};
