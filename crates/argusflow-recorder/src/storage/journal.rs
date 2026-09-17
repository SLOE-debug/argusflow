//! 长度、版本、序号和摘要校验；损坏尾部绝不猜测恢复。
use crate::{MAX_RECORD_BYTES, Record, RecordData, Session};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

/// 存储错误保留 I/O 原因，磁盘满不得报告健康状态。
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// 文件操作失败。
    #[error("录制文件操作失败：{0}")]
    Io(#[from] std::io::Error),
    /// 协议校验失败。
    #[error("录制格式错误：{0}")]
    Format(String),
    /// JSON 编解码失败。
    #[error("录制记录解析失败：{0}")]
    Json(#[from] serde_json::Error),
}
/// 存储结果。
pub type StorageResult<T> = Result<T, StorageError>;
/// 单写者日志；写失败后不可继续。
pub struct Journal {
    file: File,
    written: u64,
    synced: u64,
    failed: bool,
}
impl Journal {
    /// 只创建新的目录；不覆盖已有会话。
    pub fn create(path: &Path, session: &Session) -> StorageResult<Self> {
        std::fs::create_dir(path)?;
        std::fs::create_dir(path.join("attachments"))?;
        let mut metadata = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path.join("session.json"))?;
        metadata.write_all(&serde_json::to_vec_pretty(session)?)?;
        metadata.sync_all()?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path.join("events.afr"))?;
        Ok(Self {
            file,
            written: 0,
            synced: 0,
            failed: false,
        })
    }
    /// 完整 write_all 后才推进确认水位；失败锁定写者。
    pub fn append(&mut self, data: RecordData, qpc: i64) -> StorageResult<Record> {
        if self.failed {
            return Err(StorageError::Format("写者已故障".into()));
        }
        let id = self
            .written
            .checked_add(1)
            .ok_or_else(|| StorageError::Format("序号耗尽".into()))?;
        let record = Record {
            id,
            written_qpc: qpc,
            data,
        };
        let payload = serde_json::to_vec(&record)?;
        if payload.len() > MAX_RECORD_BYTES {
            return Err(StorageError::Format("单条记录超出1MiB预算".into()));
        }
        let mut frame = Vec::with_capacity(40 + payload.len());
        frame.extend_from_slice(b"AFR1");
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(blake3::hash(&payload).as_bytes());
        frame.extend_from_slice(&payload);
        if let Err(error) = self.file.write_all(&frame) {
            self.failed = true;
            return Err(error.into());
        }
        self.written = id;
        Ok(record)
    }
    /// 成功同步才推进持久水位。
    pub fn sync(&mut self) -> StorageResult<u64> {
        if self.failed {
            return Err(StorageError::Format("写者已故障，不能确认同步水位".into()));
        }
        if let Err(error) = self.file.sync_data() {
            self.failed = true;
            return Err(error.into());
        }
        self.synced = self.written;
        Ok(self.synced)
    }
    /// 已写入、已同步水位。
    pub fn watermarks(&self) -> (u64, u64) {
        (self.written, self.synced)
    }
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-recorder/unit/journal.rs"]
mod tests;
/// 读取分页游标，offset 必须来自上一页结果。
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Cursor {
    /// 下一记录字节位置。
    pub offset: u64,
    /// 上一条已校验序号。
    pub sequence: u64,
}
/// 校验后的前缀及明确尾部边界。
#[derive(Debug, Serialize, Deserialize)]
pub struct JournalPage {
    /// 最多请求数量的完整记录。
    pub records: Vec<Record>,
    /// 下一次读取位置。
    pub cursor: Cursor,
    /// 损坏／未完成尾部的原因；活动文件中可能只是尚未完成的 write。
    pub tail: Option<String>,
    /// 是否到达当前文件末尾。
    pub end: bool,
}
/// 顺序读取固定预算的完整记录；不读取任意超大 JSON。
pub fn read_page(path: &Path, mut cursor: Cursor, limit: usize) -> StorageResult<JournalPage> {
    let mut file = File::open(path.join("events.afr"))?;
    file.seek(SeekFrom::Start(cursor.offset))?;
    let mut page = JournalPage {
        records: vec![],
        cursor,
        tail: None,
        end: false,
    };
    let limit = limit.clamp(1, 256);
    let mut bytes = 0;
    while page.records.len() < limit && bytes < 4 * MAX_RECORD_BYTES {
        let mut header = [0u8; 40];
        let n = file.read(&mut header[..1])?;
        if n == 0 {
            page.end = true;
            break;
        }
        if file.read_exact(&mut header[1..]).is_err() {
            page.tail = Some("不完整记录头".into());
            break;
        }
        let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if &header[..4] != b"AFR1" || length == 0 || length > MAX_RECORD_BYTES {
            page.tail = Some("版本或长度校验失败".into());
            break;
        }
        let mut payload = vec![0; length];
        if file.read_exact(&mut payload).is_err() {
            page.tail = Some("不完整记录体".into());
            break;
        }
        if blake3::hash(&payload).as_bytes() != &header[8..40] {
            page.tail = Some("内容校验失败".into());
            break;
        }
        let record: Record = match serde_json::from_slice(&payload) {
            Ok(record) => record,
            Err(error) => {
                page.tail = Some(error.to_string());
                break;
            }
        };
        if record.id != cursor.sequence + 1 {
            page.tail = Some("序号不连续".into());
            break;
        }
        cursor = Cursor {
            offset: cursor.offset + 40 + length as u64,
            sequence: record.id,
        };
        bytes += length;
        page.records.push(record);
        page.cursor = cursor;
    }
    Ok(page)
}
/// 非活动会话恢复：隔离不完整尾部，原日志只保留已校验前缀。
pub fn recover(path: &Path, observed_qpc: i64) -> StorageResult<JournalPage> {
    let mut cursor = Cursor::default();
    let mut phase = None;
    loop {
        let page = read_page(path, cursor, 256)?;
        cursor = page.cursor;
        for record in &page.records {
            if let RecordData::State {
                phase: observed, ..
            } = record.data
            {
                phase = Some(observed);
            }
        }
        if page.tail.is_some() {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(path.join("events.afr"))?;
            file.seek(SeekFrom::Start(cursor.offset))?;
            let mut tail = tempfile::NamedTempFile::new_in(path)?;
            std::io::copy(&mut file, &mut tail)?;
            tail.as_file().sync_all()?;
            tail.persist_noclobber(path.join(format!("recovery-tail-{}.bin", cursor.offset)))
                .map_err(|e| StorageError::Io(e.error))?;
            file.set_len(cursor.offset)?;
            file.sync_all()?;
            std::fs::write(
                path.join("recovery.json"),
                serde_json::to_vec_pretty(&page)?,
            )?;
            append_interruption(
                path,
                cursor,
                observed_qpc,
                "恢复已隔离不完整尾部；只能保证校验后的日志前缀",
            )?;
            return Ok(page);
        }
        if page.end {
            if !matches!(
                phase,
                Some(
                    crate::SessionPhase::Stopped
                        | crate::SessionPhase::Faulted
                        | crate::SessionPhase::Interrupted
                )
            ) {
                append_interruption(
                    path,
                    cursor,
                    observed_qpc,
                    "重新打开时缺少正常终止记录；原始队列尾部与未完成证据不可恢复",
                )?;
            }
            return Ok(page);
        }
    }
}
fn append_interruption(path: &Path, cursor: Cursor, qpc: i64, reason: &str) -> StorageResult<()> {
    let file = OpenOptions::new()
        .append(true)
        .open(path.join("events.afr"))?;
    let mut journal = Journal {
        file,
        written: cursor.sequence,
        synced: 0,
        failed: false,
    };
    journal.append(
        RecordData::State {
            phase: crate::SessionPhase::Interrupted,
            reason: reason.into(),
        },
        qpc,
    )?;
    journal.sync()?;
    Ok(())
}
