//! 跨平台图像比较；不拥有采集、选帧或 OCR 策略。
mod difference;
mod view;
pub use difference::{ChangeRegion, DifferencePolicy, changed_regions, has_changes};
pub use view::ImageView;
