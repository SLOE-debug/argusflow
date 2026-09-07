//! 有界图像编码队列；摄入线程只冻结像素，不被 PNG 压缩和磁盘延迟阻塞。

use crate::{RecorderError, ScreenshotEvidence, screenshots::ScreenshotStore};
use argusflow_core::{EvidenceFrame, InspectionFailure, ScreenPoint};
use std::{
    path::PathBuf,
    sync::mpsc::{self, SyncSender},
    thread::JoinHandle,
};
use tokio::sync::oneshot;

/// worker 等待已冻结图像的保存结果，失败不能被误报成成功引用。
pub(crate) type PendingScreenshot =
    oneshot::Receiver<Result<ScreenshotEvidence, InspectionFailure>>;

/// 一项拥有冻结帧的写入请求，不访问 live UI。
struct ScreenshotJob {
    /// 关联唯一原始事件。
    sequence: u64,
    /// 完整自有像素，后台线程不再触碰屏幕。
    frame: EvidenceFrame,
    /// 相对录制起点，毫秒。
    captured_at_ms: u64,
    /// 像素冻结耗时，毫秒。
    duration_ms: u64,
    /// 屏幕物理坐标，不是裁切内坐标。
    pointer: Option<ScreenPoint>,
    /// 鼠标按下才生成局部 PNG。
    crop_click: bool,
    /// 仅返回成功落盘的引用或明确失败。
    result: oneshot::Sender<Result<ScreenshotEvidence, InspectionFailure>>,
}

/// 单次录制拥有的有界写入线程；Drop 排空已有 PNG 后结束。
pub(crate) struct ScreenshotWriter {
    /// 关闭 sender 后，独占线程排空已接收任务。
    sender: Option<SyncSender<ScreenshotJob>>,
    /// 录制摄入阶段拥有并在退出时 join。
    thread: Option<JoinHandle<()>>,
}

impl ScreenshotWriter {
    /// 仅安装编码线程，图像目录已由录制服务创建。
    pub(crate) fn start(directory: PathBuf) -> Result<Self, RecorderError> {
        // 最多四帧排队，避免高分辨率屏幕与慢磁盘导致无界内存。
        let (sender, receiver) = mpsc::sync_channel::<ScreenshotJob>(4);
        let thread = std::thread::Builder::new()
            .name("argusflow-recorder-png".into())
            .spawn(move || {
                let store = ScreenshotStore::new(directory);
                while let Ok(job) = receiver.recv() {
                    // 原生像素在后台原地转换，不能占用事件摄入线程的采样预算。
                    let frame = job.frame.into_rgba8();
                    let result = store.save(
                        job.sequence,
                        &frame,
                        job.captured_at_ms,
                        job.duration_ms,
                        job.pointer,
                        job.crop_click,
                    );
                    let _ = job.result.send(result);
                }
            })
            .map_err(|_| RecorderError::WorkerUnavailable)?;
        Ok(Self {
            sender: Some(sender),
            thread: Some(thread),
        })
    }

    /// 立即投递已冻结帧；队列满时明确失败，绝不推迟重新采样。
    pub(crate) fn submit(
        &self,
        sequence: u64,
        frame: EvidenceFrame,
        captured_at_ms: u64,
        duration_ms: u64,
        pointer: Option<ScreenPoint>,
        crop_click: bool,
    ) -> Result<PendingScreenshot, InspectionFailure> {
        let (result, receiver) = oneshot::channel();
        self.sender
            .as_ref()
            .ok_or(InspectionFailure::Unavailable)?
            .try_send(ScreenshotJob {
                sequence,
                frame,
                captured_at_ms,
                duration_ms,
                pointer,
                crop_click,
                result,
            })
            .map_err(|_| InspectionFailure::Unavailable)?;
        Ok(receiver)
    }
}

impl Drop for ScreenshotWriter {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use argusflow_core::{EvidencePixelFormat, InspectionRect};

    #[tokio::test]
    async fn writer_converts_native_pixels_before_encoding_window_and_crop() {
        let directory =
            std::env::temp_dir().join(format!("argusflow-png-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(directory.join("evidence")).unwrap();
        let writer = ScreenshotWriter::start(directory.clone()).unwrap();
        let frame = EvidenceFrame::new(
            InspectionRect {
                x: -1.0,
                y: 0.0,
                width: 2.0,
                height: 1.0,
            },
            2,
            1,
            EvidencePixelFormat::Bgrx8,
            vec![11, 22, 33, 0, 44, 55, 66, 99],
        )
        .unwrap();
        let result = writer
            .submit(1, frame, 10, 2, Some(ScreenPoint { x: 0, y: 0 }), true)
            .unwrap()
            .await
            .unwrap()
            .unwrap();
        drop(writer);
        for relative in [&result.path, &result.crop.as_ref().unwrap().path] {
            let path = directory.join(relative);
            let mut decoder =
                png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()))
                    .read_info()
                    .unwrap();
            let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
            let info = decoder.next_frame(&mut pixels).unwrap();
            assert_eq!(
                &pixels[..info.buffer_size()],
                &[33, 22, 11, 255, 66, 55, 44, 255]
            );
            drop(decoder);
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(directory.join("evidence")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[tokio::test]
    #[ignore = "explicit synthetic full-size PNG throughput benchmark; no desktop capture"]
    async fn full_size_encoding_benchmark() {
        let directory =
            std::env::temp_dir().join(format!("argusflow-png-bench-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(directory.join("evidence")).unwrap();
        let writer = ScreenshotWriter::start(directory.clone()).unwrap();
        // 合成渐变和块状图案，不读取或保存用户屏幕。
        let mut pixels = vec![0_u8; 2560 * 1549 * 4];
        for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            pixel.copy_from_slice(&[
                (index % 256) as u8,
                ((index / 2560) % 256) as u8,
                ((index / 32) % 256) as u8,
                0,
            ]);
        }
        let mut durations = Vec::new();
        for sequence in 1..=12 {
            let frame = EvidenceFrame::new(
                InspectionRect {
                    x: 0.0,
                    y: 0.0,
                    width: 2560.0,
                    height: 1549.0,
                },
                2560,
                1549,
                EvidencePixelFormat::Bgrx8,
                pixels.clone(),
            )
            .unwrap();
            let start = std::time::Instant::now();
            let evidence = writer
                .submit(sequence, frame, 0, 0, None, false)
                .unwrap()
                .await
                .unwrap()
                .unwrap();
            if sequence > 2 {
                durations.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            std::fs::remove_file(directory.join(evidence.path)).unwrap();
        }
        drop(writer);
        durations.sort_by(f64::total_cmp);
        println!(
            "synthetic PNG including conversion/write: median_ms={:.3} min_ms={:.3} max_ms={:.3}",
            (durations[4] + durations[5]) / 2.0,
            durations[0],
            durations[9]
        );
        std::fs::remove_dir(directory.join("evidence")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
