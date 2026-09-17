//! 内容寻址附件只有完整发布后才返回引用。
use super::{StorageError, StorageResult};
use crate::{Attachment, Session};
use std::{
    io::Write,
    path::{Component, Path},
};
/// 验证只包含单个会话目录名。
pub fn session_path(root: &Path, id: &str) -> StorageResult<std::path::PathBuf> {
    if id.is_empty()
        || id.len() > 128
        || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(StorageError::Format("会话身份无效".into()));
    }
    Ok(root.join(id))
}
/// 读取并校验当前版本元数据。
pub fn load_session(path: &Path) -> StorageResult<Session> {
    let file = std::fs::File::open(path.join("session.json"))?;
    if file.metadata()?.len() > 65536 {
        return Err(StorageError::Format("元数据超限".into()));
    }
    let session: Session = serde_json::from_reader(file)?;
    if session.format != 2 || session.qpc_frequency == 0 {
        return Err(StorageError::Format("不支持的格式或时钟".into()));
    }
    Ok(session)
}
/// 发布编码 PNG，64MiB 单图、2GiB 会话预算由调用方协调。
pub fn publish_image(path: &Path, bytes: &[u8], size: [u32; 2]) -> StorageResult<Attachment> {
    publish_in(path, "attachments", bytes, size)
}
pub(super) fn publish_in(
    path: &Path,
    folder: &str,
    bytes: &[u8],
    size: [u32; 2],
) -> StorageResult<Attachment> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(StorageError::Format("图片预算不足".into()));
    }
    let hash = blake3::hash(bytes).to_hex().to_string();
    let relative = format!("{folder}/{hash}.png");
    let destination = path.join(&relative);
    if destination.exists() {
        if blake3::hash(&std::fs::read(&destination)?)
            .to_hex()
            .as_str()
            != hash
        {
            return Err(StorageError::Format("附件内容损坏".into()));
        }
    } else {
        let mut temporary = tempfile::NamedTempFile::new_in(path.join(folder))?;
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
        match temporary.persist_noclobber(&destination) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                if std::fs::read(&destination)? != bytes {
                    return Err(StorageError::Format("并发附件内容冲突".into()));
                }
            }
            Err(error) => return Err(error.error.into()),
        }
    }
    Ok(Attachment {
        hash,
        path: relative,
        bytes: bytes.len() as u64,
        size,
    })
}
/// 只允许正常的相对附件路径，读取时核对哈希和长度。
pub fn read_attachment(path: &Path, attachment: &Attachment) -> StorageResult<Vec<u8>> {
    let relative = Path::new(&attachment.path);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
        || (attachment.path != format!("attachments/{}.png", attachment.hash)
            && attachment.path != format!("review-cache/{}.png", attachment.hash))
        || attachment.hash.len() != 64
        || !attachment.hash.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(StorageError::Format("附件路径无效".into()));
    }
    let target = path.join(relative);
    if target.metadata()?.len() != attachment.bytes || attachment.bytes > 64 * 1024 * 1024 {
        return Err(StorageError::Format("附件长度不匹配".into()));
    }
    let bytes = std::fs::read(target)?;
    if blake3::hash(&bytes).to_hex().as_str() != attachment.hash {
        return Err(StorageError::Format("附件校验失败".into()));
    }
    Ok(bytes)
}
