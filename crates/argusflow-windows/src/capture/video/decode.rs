//! 回看只解码指定视频时间的单帧，核对真实PTS后交付CPU像素。
use super::{
    model::{Result, VideoError, required},
    runtime::Runtime,
};
use std::path::Path;
use windows::{
    Win32::{Media::MediaFoundation::*, System::Com::StructuredStorage::PROPVARIANT},
    core::{GUID, HSTRING},
};

/// 已核对PTS的顶向下RGBA图片，仅用于按需回看。
pub struct DecodedVideoFrame {
    /// 图片宽度。
    pub width: u32,
    /// 图片高度。
    pub height: u32,
    /// 顶向下紧凑RGBA像素。
    pub rgba: Vec<u8>,
}
/// 从前一个关键帧向后解码；不把seek返回的第一帧冒充目标帧。
pub fn decode_video_frame(path: &Path, pts: i64) -> Result<DecodedVideoFrame> {
    VideoDecoder::open(path)?.read(pts)
}

/// 单线程复用Source Reader；按相邻时间顺序解码，回退或远跳才seek。
/// 包含线程所属COM运行时，不允许移动到其他线程。
pub struct VideoDecoder {
    reader: IMFSourceReader,
    size: u64,
    stride: i32,
    buffer_height: u32,
    last: Option<i64>,
    _runtime: Runtime,
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl VideoDecoder {
    /// 打开一个视频片段；释放reader后再释放本线程COM/MF运行时。
    pub fn open(path: &Path) -> Result<Self> {
        let runtime = Runtime::new()?;
        // SAFETY: 此线程独占source reader，类型协商后检查尺寸、步长和缓冲长度。
        unsafe {
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 1)?;
            let attributes = required(attributes, "reader attributes")?;
            let reader =
                MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attributes)?;
            let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(stream, true)?;
            let media = MFCreateMediaType()?;
            media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            media.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
            reader.SetCurrentMediaType(stream, None, &media)?;
            let media = reader.GetCurrentMediaType(stream)?;
            let size = media.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let (width, height) = ((size >> 32) as u32, size as u32);
            if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 3840 * 2160 {
                return Err(VideoError::Invalid("视频尺寸超过回看预算".into()));
            }
            let stride = media.GetUINT32(&MF_MT_DEFAULT_STRIDE)? as i32;
            Ok(Self {
                reader,
                size,
                stride,
                buffer_height: height,
                last: None,
                _runtime: runtime,
                _thread: std::marker::PhantomData,
            })
        }
    }
    /// 核对实际PTS后返回像素；顺序请求复用解码状态。
    pub fn read(&mut self, pts: i64) -> Result<DecodedVideoFrame> {
        self.read_pixels(pts, |bytes, width, height, buffer_height, stride| {
            Ok(DecodedVideoFrame {
                width,
                height,
                rgba: super::pixels::rgba(bytes, width, height, buffer_height, stride)?,
            })
        })
    }
    /// 顺序提取分析小图；中间依赖帧仅解码，不转换RGB。
    pub fn thumbnail(&mut self, pts: i64) -> Result<super::VideoThumbnail> {
        self.read_pixels(pts, super::thumbnail::thumbnail)
    }
    fn read_pixels<T>(
        &mut self,
        pts: i64,
        convert: impl FnOnce(&[u8], u32, u32, u32, i32) -> Result<T>,
    ) -> Result<T> {
        if pts < 0 {
            return Err(VideoError::Invalid("视频时间不能为负数".into()));
        }
        let reader = &self.reader;
        let size = self.size;
        let (width, height) = ((size >> 32) as u32, size as u32);
        let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        // SAFETY: reader及运行时属于本线程，像素缓冲长度在访问前验证。
        unsafe {
            if !self
                .last
                .is_some_and(|last| pts > last && pts - last <= 10_000_000)
            {
                reader.SetCurrentPosition(&GUID::zeroed(), &PROPVARIANT::from(pts))?;
            }
            self.last = None;
            for _ in 0..600 {
                let mut flags = 0;
                let mut time = 0;
                let mut sample = None;
                reader.ReadSample(
                    stream,
                    0,
                    None,
                    Some(&mut flags),
                    Some(&mut time),
                    Some(&mut sample),
                )?;
                if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                    let current = reader.GetCurrentMediaType(stream)?;
                    if current.GetGUID(&MF_MT_SUBTYPE)? != MFVideoFormat_NV12 {
                        return Err(VideoError::Invalid("视频解码像素类型改变".into()));
                    }
                    let coded = current.GetUINT64(&MF_MT_FRAME_SIZE)?;
                    if coded != size {
                        // H264按宏块补齐高度，显示区域才是有效像素，不展示填充行。
                        let mut aperture = MFVideoArea::default();
                        let blob = std::slice::from_raw_parts_mut(
                            (&mut aperture as *mut MFVideoArea).cast::<u8>(),
                            std::mem::size_of::<MFVideoArea>(),
                        );
                        current.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, blob, None)?;
                        if aperture.Area.cx != width as i32
                            || aperture.Area.cy != height as i32
                            || aperture.OffsetX.value != 0
                            || aperture.OffsetY.value != 0
                            || aperture.OffsetX.fract != 0
                            || aperture.OffsetY.fract != 0
                            || (coded >> 32) < u64::from(width)
                            || (coded as u32) < height
                            || (coded as u32) > height + 32
                        {
                            return Err(VideoError::Invalid("视频显示区域与原始尺寸不一致".into()));
                        }
                    }
                    self.buffer_height = coded as u32;
                    self.stride = current.GetUINT32(&MF_MT_DEFAULT_STRIDE)? as i32;
                }
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
                let Some(sample) = sample else {
                    continue;
                };
                if time < pts {
                    continue;
                }
                if time != pts {
                    return Err(VideoError::Invalid(format!(
                        "视频帧尚未写入或索引不匹配：请求{pts}，解码{time}"
                    )));
                }
                let buffer = sample.ConvertToContiguousBuffer()?;
                let mut data = std::ptr::null_mut();
                let mut length = 0;
                buffer.Lock(&mut data, None, Some(&mut length))?;
                let bytes = std::slice::from_raw_parts(data, length as usize);
                let copy = convert(bytes, width, height, self.buffer_height, self.stride);
                buffer.Unlock()?;
                self.last = Some(time);
                return copy;
            }
            Err(VideoError::Invalid(
                "此帧尚未写入视频，暂停或停止录制后重试".into(),
            ))
        }
    }
}
