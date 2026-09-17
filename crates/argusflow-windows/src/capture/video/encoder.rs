//! Media Foundation 文件写入边界；只接受 NV12 GPU 样本并验证硬件编码器。
use super::{
    model::{Result, VideoError, required},
    sample,
};
use std::{path::Path, time::Instant};
use windows::{
    Win32::{Graphics::Direct3D11::ID3D11Device, Media::MediaFoundation::*},
    core::{HSTRING, Interface},
};

pub(super) struct Encoder {
    writer: IMFSinkWriter,
    _sink: Sink,
    stream: u32,
    codec: ICodecAPI,
    last_key: i64,
    pub hardware: String,
    pub max_write_ms: u128,
}
struct Sink(IMFMediaSink);
impl Drop for Sink {
    fn drop(&mut self) {
        // SAFETY: 包括构造失败在内的所有路径均释放 sink 内部资源。
        let _ = unsafe { self.0.Shutdown() };
    }
}
fn media_type(
    width: u32,
    height: u32,
    fps: u32,
    encoded: bool,
    bitrate: u32,
) -> Result<IMFMediaType> {
    // SAFETY: 独占媒体类型配置，大小/速率已经由会话验证。
    unsafe {
        let media = MFCreateMediaType()?;
        media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        media.SetGUID(
            &MF_MT_SUBTYPE,
            if encoded {
                &MFVideoFormat_H264
            } else {
                &MFVideoFormat_NV12
            },
        )?;
        media.SetUINT64(
            &MF_MT_FRAME_SIZE,
            (u64::from(width) << 32) | u64::from(height),
        )?;
        media.SetUINT64(&MF_MT_FRAME_RATE, (u64::from(fps) << 32) | 1)?;
        media.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
        media.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        media.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32)?;
        media.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
        media.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)?;
        if encoded {
            media.SetUINT32(&MF_MT_AVG_BITRATE, bitrate)?;
        }
        Ok(media)
    }
}
impl Encoder {
    pub fn new(
        device: &ID3D11Device,
        path: &Path,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
    ) -> Result<Self> {
        // SAFETY: MTA 工作线程拥有 writer；所有设置在 BeginWriting 前完成。
        unsafe {
            let mut manager = None;
            let mut token = 0;
            MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
            let manager = required(manager, "DXGI device manager")?;
            manager.ResetDevice(device, token)?;
            let mut attrs = None;
            MFCreateAttributes(&mut attrs, 4)?;
            let attrs = required(attrs, "sink writer attributes")?;
            attrs.SetUnknown(&MF_SINK_WRITER_D3D_MANAGER, &manager)?;
            attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
            attrs.SetUINT32(&MF_READWRITE_D3D_OPTIONAL, 0)?;
            attrs.SetUINT32(&MF_LOW_LATENCY, 1)?;
            let output = media_type(width, height, fps, true, bitrate)?;
            let input = media_type(width, height, fps, false, bitrate)?;
            let file = MFCreateFile(
                MF_ACCESSMODE_WRITE,
                MF_OPENMODE_FAIL_IF_EXIST,
                MF_FILEFLAGS_NONE,
                &HSTRING::from(path.as_os_str()),
            )?;
            let sink = Sink(MFCreateFMPEG4MediaSink(&file, &output, None)?);
            let writer = MFCreateSinkWriterFromMediaSink(&sink.0, &attrs)?;
            // 已配置的视频 sink 自带 stream 0，不能再次 AddStream。
            let stream = 0;
            writer.SetInputMediaType(stream, &input, None)?;
            let extended: IMFSinkWriterEx = writer.cast()?;
            let mut hardware = None;
            let mut codec = None;
            for index in 0..8 {
                let mut transform = None;
                if extended
                    .GetTransformForStream(stream, index, None, &mut transform)
                    .is_err()
                {
                    break;
                }
                if let Some(transform) = transform
                    && let Ok(attributes) = transform.GetAttributes()
                    && let Ok(length) = attributes.GetStringLength(&MFT_ENUM_HARDWARE_URL_Attribute)
                {
                    let mut text = vec![0u16; length as usize + 1];
                    attributes.GetString(&MFT_ENUM_HARDWARE_URL_Attribute, &mut text, None)?;
                    hardware = Some(String::from_utf16_lossy(&text[..length as usize]));
                    codec = Some(transform.cast::<ICodecAPI>()?);
                }
            }

            let hardware = hardware.ok_or_else(|| {
                VideoError::Invalid("未能确认实际硬件编码器，拒绝软件编码".into())
            })?;
            let codec = required(codec, "hardware codec controls")?;
            use windows::Win32::System::Variant::VARIANT;
            // 单遍受峰值约束VBR：静态办公画面不必始终填满目标码率。
            codec.SetValue(
                &CODECAPI_AVEncCommonRateControlMode,
                &VARIANT::from(eAVEncCommonRateControlMode_PeakConstrainedVBR.0 as u32),
            )?;
            codec.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &VARIANT::from(bitrate))?;
            codec.SetValue(
                &CODECAPI_AVEncCommonMaxBitRate,
                &VARIANT::from(bitrate.saturating_mul(2)),
            )?;
            codec.SetValue(&CODECAPI_AVLowLatencyMode, &VARIANT::from(true))?;
            writer.BeginWriting()?;
            Ok(Self {
                writer,
                _sink: sink,
                stream,
                codec,
                last_key: -10_000_000,
                hardware,
                max_write_ms: 0,
            })
        }
    }
    pub fn write(
        &mut self,
        frame: &super::journal::PendingFrame,
        duration: i64,
        callback: &IMFAsyncCallback,
    ) -> Result<()> {
        let sample = sample::sample(&frame.texture, frame.entry.pts_100ns, duration, callback)?;
        let started = Instant::now();
        if frame.entry.pts_100ns - self.last_key >= 10_000_000 {
            // SAFETY: 强制按墙钟约1秒建立关键帧，静态画面也能及时封装分片。
            unsafe {
                self.codec.SetValue(
                    &CODECAPI_AVEncVideoForceKeyFrame,
                    &windows::Win32::System::Variant::VARIANT::from(true),
                )?;
            }
            self.last_key = frame.entry.pts_100ns;
        }
        // SAFETY: 带时间戳的样本只提交一次；纹理生命周期由 tracked sample 管理。
        unsafe {
            self.writer
                .WriteSample(self.stream, &sample)
                .map_err(|source| VideoError::Stage {
                    stage: "WriteSample",
                    source,
                })?;
        }
        self.max_write_ms = self.max_write_ms.max(started.elapsed().as_millis());
        Ok(())
    }
    pub fn finish(&self) -> Result<()> {
        // SAFETY: 所有输入已提交，Finalize 完成之后才能报告成功。
        unsafe {
            self.writer.Finalize().map_err(|source| VideoError::Stage {
                stage: "Finalize",
                source,
            })?;
        }
        Ok(())
    }
}
