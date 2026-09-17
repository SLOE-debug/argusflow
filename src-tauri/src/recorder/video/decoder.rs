//! 唯一回看线程持有COM解码器，30秒空闲后释放；通道不积压长录制帧。
use argusflow_windows::{DecodedVideoFrame, VideoDecoder, VideoThumbnail};
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Default)]
pub(super) struct Decoder {
    opened: Option<(PathBuf, u64, SystemTime, VideoDecoder)>,
}
impl Decoder {
    pub fn read(&mut self, path: &Path, pts: i64) -> Result<DecodedVideoFrame, String> {
        self.with(path, |decoder| decoder.read(pts))
    }
    pub fn thumbnail(&mut self, path: &Path, pts: i64) -> Result<VideoThumbnail, String> {
        self.with(path, |decoder| decoder.thumbnail(pts))
    }
    fn with<T>(
        &mut self,
        path: &Path,
        read: impl FnOnce(&mut VideoDecoder) -> Result<T, argusflow_windows::VideoError>,
    ) -> Result<T, String> {
        let metadata = path.metadata().map_err(|e| e.to_string())?;
        let modified = metadata.modified().map_err(|e| e.to_string())?;
        if !self.opened.as_ref().is_some_and(|(p, len, time, _)| {
            p == path && *len == metadata.len() && *time == modified
        }) {
            self.opened = None;
            self.opened = Some((
                path.to_owned(),
                metadata.len(),
                modified,
                VideoDecoder::open(path).map_err(|e| e.to_string())?,
            ));
        }
        let result =
            read(&mut self.opened.as_mut().ok_or("解码器未打开")?.3).map_err(|e| e.to_string());
        if result.is_err() {
            self.opened = None;
        }
        result
    }
}
