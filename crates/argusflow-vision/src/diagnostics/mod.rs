//! 仅在显式配置目录时持久化视觉失败现场，避免正常运行泄漏窗口像素。

mod ocr_input;

pub(crate) use ocr_input::persist_scene_timeout;
