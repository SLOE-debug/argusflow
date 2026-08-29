//! 仅在显式配置目录时持久化视觉失败现场，避免正常运行泄漏窗口像素。

mod ocr_input;

pub use ocr_input::encode_bgra_as_bmp;
