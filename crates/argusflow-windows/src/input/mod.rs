//! 独立的真实输入能力，不占用 UIA apartment。
mod inject;
mod keyboard;
mod sequence;
mod service;
mod submission;
pub use sequence::InputSequence;
pub use service::{InputAction, InputService, InputState};
