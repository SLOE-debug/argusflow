//! 有限时长原型会话编排；采集和编码线程只交换有界 GPU 帧。
use super::{
    encoder::Encoder,
    graphics::Graphics,
    journal::{Journal, Message},
    model::{Result, VideoError, VideoOptions, VideoReport},
    runtime::Runtime,
    timing::{pts, qpc},
    writer::encode,
};
use crate::capture::{clock::Clock, topology};
use std::{sync::mpsc, time::Duration};
use windows::Win32::Graphics::Dxgi::*;

/// 每屏8张原生NV12纹理；包含排队、待补duration和编码器仍持有的样本。
const SLOTS: usize = 8;
/// 阻塞录制指定显示器的 SDR 原生分辨率视频及 QPC 索引。
/// 此原型需由受监督的独立进程调用，以隔离不能取消的驱动/COM调用。
/// 不改变现有 UI 录制入口；不录音、不裁剪，也不自动注入用户输入。
pub fn record_desktop_video(options: VideoOptions) -> Result<VideoReport> {
    record_desktop_video_until(options, &std::sync::atomic::AtomicBool::new(false))
}
/// 录制直到外部停止标记或时长上限；就绪文件在首帧提交后发布。
pub fn record_desktop_video_until(
    options: VideoOptions,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<VideoReport> {
    if !(1..=60).contains(&options.fps)
        || options.bitrate == 0
        || options.max_file_bytes == 0
        || options.duration.is_zero()
        || options.duration > Duration::from_secs(3600)
    {
        return Err(VideoError::Invalid(
            "帧率需1..60，码率/配额/时长非零且时长不超过1小时".into(),
        ));
    }
    let _runtime = Runtime::new()?;
    let _dpi = crate::capture::dpi::DpiScope::enter();
    // SAFETY: DXGI 输出由本次枚举获得，不使用缓存句柄。
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
    let adapter = unsafe { factory.EnumAdapters1(options.adapter)? };
    let output = topology::outputs(&adapter, options.adapter, 1)?
        .into_iter()
        .nth(options.output)
        .ok_or_else(|| VideoError::Invalid("指定显示输出不存在".into()))?;
    if output.info.state == argusflow_capture_contracts::SourceState::Unavailable
        || output.info.rotation != argusflow_capture_contracts::Rotation::Identity
    {
        return Err(VideoError::Invalid("原型仅支持未旋转的 SDR 输出".into()));
    }
    let (width, height) = (output.raw_width, output.raw_height);
    let texture_bytes = u64::from(width) * u64::from(height) * 3 / 2 * (SLOTS + 1) as u64;
    if width % 2 != 0 || height % 2 != 0 || texture_bytes > 256 * 1024 * 1024 {
        return Err(VideoError::Invalid(
            "NV12尺寸必须为偶数且纹理池不得超过256MiB".into(),
        ));
    }
    let graphics = Graphics::new(&adapter, width, height, options.fps)?;
    let duplication = unsafe { output.output.DuplicateOutput(&graphics.device)? };
    let (returned, available) = mpsc::sync_channel(SLOTS);
    for _ in 0..SLOTS {
        returned
            .send(graphics.texture()?)
            .map_err(|_| VideoError::Invalid("纹理池关闭".into()))?;
    }
    std::fs::create_dir(&options.directory)?;
    let mut gaps = Journal::new(&options.directory.join("capture.jsonl"))?;
    let device = graphics.device.clone();
    let (sender, receiver) = mpsc::sync_channel(SLOTS);
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let clock = Clock::new()?;
    let video = options.directory.join("screen.mp4");
    let mut header = Journal::new(&options.directory.join("session.json"))?;
    header.write(&serde_json::json!({
        "format":"native-video-prototype-1",
        "qpc_origin":clock.0.origin, "qpc_frequency":clock.0.frequency,
        "fps_limit":options.fps, "bitrate":options.bitrate,
        "requested_rate_control":"peak_constrained_vbr",
        "requested_peak_bitrate":options.bitrate.saturating_mul(2),
        "max_file_bytes":options.max_file_bytes,
        "width":width, "height":height,
        "source":{
            "id":output.info.id.0, "name":output.info.name,
            "origin":[output.info.bounds.x(),output.info.bounds.y()],
            "dpi":[output.info.dpi.0,output.info.dpi.1]
        },
        "cursor":"DXGI hardware pointer not composited; input recording not integrated",
        "pts_unit":"100ns", "media_quantum_100ns":10000,
        "bootstrap":"first acquired frame is the baseline at PTS zero; original QPC retained"
    }))?;
    header.sync()?;
    std::thread::scope(|scope| -> Result<VideoReport> {
        let options_ref = &options;
        let worker = scope.spawn(move || -> Result<(u64, String, u128)> {
            let options = options_ref;
            let _runtime = Runtime::new()?;
            let result = Encoder::new(&device, &video, width, height, options.fps, options.bitrate);
            let _ = ready_tx.send(result.as_ref().map(|_| ()).map_err(ToString::to_string));
            let mut encoder = result?;
            encode(
                &mut encoder,
                &receiver,
                returned,
                &options.directory,
                options.fps,
            )?;
            Ok((
                std::fs::metadata(&video)?.len(),
                encoder.hardware.clone(),
                encoder.max_write_ms,
            ))
        });
        ready_rx
            .recv()
            .map_err(|_| VideoError::Invalid("编码线程初始化中断".into()))?
            .map_err(VideoError::Invalid)?;
        let mut report = VideoReport {
            frames: 0,
            pool_drops: 0,
            accumulated_updates: 0,
            texture_bytes,
            max_write_ms: 0,
            hardware: String::new(),
            video_bytes: 0,
        };
        let result = super::capture_loop::CaptureLoop {
            graphics: &graphics,
            duplication: &duplication,
            available: &available,
            sender: &sender,
            gaps: &mut gaps,
            clock,
            options: &options,
            report: &mut report,
            stop,
        }
        .run();
        // 即使采集失败也结束已提交前缀；result仍向外传播，不能报告成功。
        let _ = sender.send(Message::End(super::timing::media_pts(pts(qpc()?, clock))));
        drop(sender);
        let encoded = worker
            .join()
            .map_err(|_| VideoError::Invalid("编码线程崩溃".into()))?;
        gaps.write(&serde_json::json!({"end_qpc":qpc()?, "capture_error":result.as_ref().err().map(ToString::to_string)}))?;
        gaps.sync()?;
        let (bytes, hardware, max_write_ms) = encoded?;
        result?;
        report.hardware = hardware;
        report.max_write_ms = max_write_ms;
        report.video_bytes = bytes;
        let mut summary = Journal::new(&options.directory.join("complete.json"))?;
        summary.write(&report)?;
        summary.sync()?;
        Ok(report)
    })
}
