//! GPU 帧与 JSONL 索引；索引表示提交事实，完成水位另行发布。
use super::model::Result;
use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
};
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;

#[derive(Serialize)]
pub(super) struct FrameEntry {
    pub sequence: u64,
    pub presented_qpc: i64,
    pub acquired_qpc: i64,
    /// 相对会话起点的100ns时间，与视频 PTS 一致。
    pub pts_100ns: i64,
    pub accumulated: u32,
    /// 静态心跳只延长上一画面的有效区间，不是新的桌面呈现。
    pub repeated: bool,
}
pub(super) struct PendingFrame {
    pub texture: ID3D11Texture2D,
    pub entry: FrameEntry,
}
pub(super) struct Journal(BufWriter<File>);
impl Journal {
    pub fn new(path: &Path) -> Result<Self> {
        Ok(Self(BufWriter::new(
            OpenOptions::new().write(true).create_new(true).open(path)?,
        )))
    }
    pub fn write(&mut self, value: &impl Serialize) -> Result<()> {
        serde_json::to_writer(&mut self.0, value)?;
        self.0.write_all(b"\n")?;
        self.0.flush()?;
        Ok(())
    }
    pub fn sync(&mut self) -> Result<()> {
        self.0.flush()?;
        self.0.get_ref().sync_all()?;
        Ok(())
    }
}

pub(super) enum Message {
    Frame(PendingFrame),
    End(i64),
}
