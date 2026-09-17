//! 私有管道读写，所有消息长度有界。
use argusflow_recorder::MAX_RECORD_BYTES;
use serde::{Serialize, de::DeserializeOwned};
use std::io::{BufRead, Write};
pub(super) fn read<T: DeserializeOwned>(reader: &mut impl BufRead) -> Result<Option<T>, String> {
    let mut bytes = Vec::new();
    loop {
        let buffer = reader.fill_buf().map_err(|e| e.to_string())?;
        if buffer.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err("IPC 尾部不完整".into())
            };
        }
        let end = buffer.iter().position(|b| *b == b'\n').map(|i| i + 1);
        let size = end.unwrap_or(buffer.len());
        if bytes.len() + size > MAX_RECORD_BYTES {
            return Err("IPC 消息超过1MiB".into());
        }
        bytes.extend_from_slice(&buffer[..size]);
        reader.consume(size);
        if end.is_some() {
            return serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| e.to_string());
        }
    }
}
pub(super) fn write(writer: &mut impl Write, message: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec(message).map_err(|e| e.to_string())?;
    if bytes.len() + 1 > MAX_RECORD_BYTES {
        return Err("IPC 消息超过预算".into());
    }
    writer
        .write_all(&bytes)
        .and_then(|_| writer.write_all(b"\n"))
        .and_then(|_| writer.flush())
        .map_err(|e| e.to_string())
}
