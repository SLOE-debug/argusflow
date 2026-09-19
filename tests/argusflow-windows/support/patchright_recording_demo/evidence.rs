//! 聊天区域截图证据。
use super::{
    Result,
    wechat_support::{frame::Frame, layout::Zone},
};
use std::path::Path;

/// 只保存聊天正文及编辑区，排除左侧其他会话。
pub fn save_frame(frame: &Frame, path: &Path) -> Result<()> {
    let region = Zone::Messages.region(frame.bounds)?;
    let header = Zone::Header.region(frame.bounds)?;
    let editor = Zone::Editor.region(frame.bounds)?;
    let crop = argusflow_capture_contracts::PixelRect::new(
        region.x(),
        header.y(),
        region.width(),
        editor.bottom() - header.y(),
    )?;
    let pixels = frame.crop(crop)?;
    let stride = (crop.width() as usize * 3 + 3) & !3;
    let image_size = stride * crop.height() as usize;
    let mut bmp = Vec::with_capacity(54 + image_size);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((54 + image_size) as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&54u32.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(crop.width() as i32).to_le_bytes());
    bmp.extend_from_slice(&(-(crop.height() as i32)).to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&24u16.to_le_bytes());
    bmp.extend_from_slice(&[0; 24]);
    for row in pixels.chunks_exact(crop.width() as usize * 3) {
        bmp.extend_from_slice(row);
        bmp.resize(bmp.len() + stride - row.len(), 0);
    }
    std::fs::write(path, bmp)?;
    Ok(())
}
