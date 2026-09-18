//! Windows 安装包资源位于可执行文件旁；开发构建由 Tauri 同步同样的目录结构。
use std::path::{Path, PathBuf};
/// 固定应用资源路径，不读取工作流参数，不回退到工作目录或用户安装的推理库。
pub(super) fn ocr_directory() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| format!("无法定位应用资源：{e}"))?;
    beside_executable(&executable)
}
fn beside_executable(executable: &Path) -> Result<PathBuf, String> {
    let directory = executable.parent().ok_or("无法定位应用安装目录")?;
    Ok(directory.join("ocr"))
}

#[cfg(test)]
#[path = "../../../tests/argusflow-desktop/unit/bundled_resources.rs"]
mod tests;
