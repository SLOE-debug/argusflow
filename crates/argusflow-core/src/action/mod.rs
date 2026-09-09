//! 跨能力共享的坐标和输入参数。
mod geometry;
mod input;
pub use geometry::{CssPoint, ImagePoint, ScreenPoint};
pub use input::{ClickCount, Key, MouseButton, ScrollAxis};
