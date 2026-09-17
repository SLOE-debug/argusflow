//! 派生图片缓存可重建；只淘汰本目录中已验证命名的PNG，不触及原始证据。
use super::{StorageError, StorageResult, package::publish_in};
use crate::Attachment;
use std::path::Path;
/// 发布按需解码图片，保留最近32张且不超过256MiB；调用方串行化发布。
pub fn publish_review_image(
    root: &Path,
    bytes: &[u8],
    size: [u32; 2],
) -> StorageResult<Attachment> {
    let cache = root.join("review-cache");
    std::fs::create_dir_all(&cache)?;
    if std::fs::symlink_metadata(&cache)?.file_type().is_symlink() {
        return Err(StorageError::Format("回看缓存不能为链接".into()));
    }
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(StorageError::Format("单帧图片超限".into()));
    }
    let name = format!("{}.png", blake3::hash(bytes).to_hex());
    let mut files = vec![];
    let mut total = bytes.len() as u64;
    for entry in std::fs::read_dir(&cache)? {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().into_owned();
        if !entry.file_type()?.is_file()
            || !filename.is_ascii()
            || filename.len() != 68
            || !filename.ends_with(".png")
            || !filename[..64].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(StorageError::Format(
                "回看缓存包含未知文件，拒绝清理".into(),
            ));
        }
        if filename == name {
            continue;
        }
        let metadata = entry.metadata()?;
        total = total.saturating_add(metadata.len());
        files.push((metadata.modified()?, entry.path(), metadata.len()));
        if files.len() > 256 {
            return Err(StorageError::Format("回看缓存文件数量异常".into()));
        }
    }
    files.sort_by_key(|f| f.0);
    let mut count = files.len() + 1;
    for (_, path, len) in files {
        if count <= 32 && total <= 256 * 1024 * 1024 {
            break;
        }
        std::fs::remove_file(path)?;
        total -= len;
        count -= 1;
    }
    let image = publish_in(root, "review-cache", bytes, size)?;
    std::fs::File::options()
        .write(true)
        .open(root.join(&image.path))?
        .set_modified(std::time::SystemTime::now())?;
    Ok(image)
}
#[cfg(test)]
#[path = "../../../../tests/argusflow-recorder/unit/review_cache.rs"]
mod tests;
