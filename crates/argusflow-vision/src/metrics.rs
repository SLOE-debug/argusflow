//! 视觉管线可观测指标；优化前先保留可验证数据。

use std::{
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use serde::{Deserialize, Serialize};

/// VisionRuntime 的无锁计数器集合。
#[derive(Debug, Default)]
pub struct VisionMetrics {
    /// 首次接受捕获帧时开始的本地测量窗口。
    started_at: OnceLock<std::time::Instant>,
    /// 已接收的捕获帧数。
    capture_frames: AtomicU64,
    /// 采样过的像素数量。
    captured_pixels: AtomicU64,
    /// OCR 实际处理的 ROI 像素数量。
    ocr_processed_pixels: AtomicU64,
    /// 发起的 tiny OCR 请求数。
    tiny_requests: AtomicU64,
    /// 发起的 medium OCR 请求数。
    medium_requests: AtomicU64,
    /// 被 generation 取消的旧请求数。
    cancelled_stale_requests: AtomicU64,
    /// 成功构建的 scene 数量。
    scenes_built: AtomicU64,
    /// 完成的 scene query 数量。
    scene_queries: AtomicU64,
    /// 最近一次稳定帧的变化比例，按百万分之一存储。
    last_changed_area_ratio_ppm: AtomicU64,
    /// 已观测的稳定帧差分次数。
    diff_observations: AtomicU64,
    /// 稳定帧门控累计耗时，单位为毫秒。
    stable_frame_latency_ms: AtomicU64,
    /// tiny OCR 累计耗时，单位为毫秒。
    tiny_latency_ms: AtomicU64,
    /// medium OCR 累计耗时，单位为毫秒。
    medium_latency_ms: AtomicU64,
    /// scene merge 累计耗时，单位为毫秒。
    scene_merge_latency_ms: AtomicU64,
    /// scene query 累计耗时，单位为毫秒。
    scene_query_latency_ms: AtomicU64,
    /// 观测到的 worker 最大排队深度。
    max_worker_queue_depth: AtomicU64,
}

impl VisionMetrics {
    /// 记录一张捕获帧及其像素规模。
    pub fn record_capture(&self, pixels: u64) {
        let _ = self.started_at.get_or_init(std::time::Instant::now);
        self.capture_frames.fetch_add(1, Ordering::Relaxed);
        self.captured_pixels.fetch_add(pixels, Ordering::Relaxed);
    }

    /// 返回当前测量窗口内的捕获帧率；没有捕获帧时返回零。
    pub fn capture_fps(&self) -> f64 {
        let frames = self.capture_frames.load(Ordering::Relaxed);
        let Some(started_at) = self.started_at.get() else {
            return 0.0;
        };
        let elapsed_seconds = started_at.elapsed().as_secs_f64();
        if elapsed_seconds <= f64::EPSILON {
            0.0
        } else {
            frames as f64 / elapsed_seconds
        }
    }

    /// 记录一次 OCR ROI 处理。
    pub fn record_ocr(&self, model: crate::ocr::OcrModel, pixels: u64) {
        self.ocr_processed_pixels
            .fetch_add(pixels, Ordering::Relaxed);
        match model {
            crate::ocr::OcrModel::PpOcrV6Tiny => {
                self.tiny_requests.fetch_add(1, Ordering::Relaxed);
            }
            crate::ocr::OcrModel::PpOcrV6Medium => {
                self.medium_requests.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// 记录一项旧 generation OCR 结果被丢弃。
    pub fn record_cancelled_stale_request(&self) {
        self.cancelled_stale_requests
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 记录新场景构建。
    pub fn record_scene_built(&self) {
        self.scenes_built.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录一次 scene query。
    pub fn record_scene_query(&self) {
        self.scene_queries.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录一份差分结果；原子整数避免在共享指标中直接存储浮点值。
    pub fn record_diff(&self, changed_area_ratio: f32) {
        let ratio_ppm = (changed_area_ratio.clamp(0.0, 1.0) * 1_000_000.0).round() as u64;
        self.last_changed_area_ratio_ppm
            .store(ratio_ppm, Ordering::Relaxed);
        self.diff_observations.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录稳定帧门控耗时。
    pub fn record_stable_frame_latency(&self, elapsed: Duration) {
        self.stable_frame_latency_ms
            .fetch_add(duration_millis(elapsed), Ordering::Relaxed);
    }

    /// 记录一次 OCR 的端到端耗时。
    pub fn record_ocr_latency(&self, model: crate::ocr::OcrModel, elapsed: Duration) {
        let target = match model {
            crate::ocr::OcrModel::PpOcrV6Tiny => &self.tiny_latency_ms,
            crate::ocr::OcrModel::PpOcrV6Medium => &self.medium_latency_ms,
        };
        target.fetch_add(duration_millis(elapsed), Ordering::Relaxed);
    }

    /// 记录 worker 当前的排队深度峰值。
    pub fn record_worker_queue_depth(&self, depth: usize) {
        let depth = depth as u64;
        let mut observed = self.max_worker_queue_depth.load(Ordering::Relaxed);
        while depth > observed {
            match self.max_worker_queue_depth.compare_exchange_weak(
                observed,
                depth,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(previous) => observed = previous,
            }
        }
    }

    /// 记录 scene query 的端到端耗时。
    pub fn record_scene_query_latency(&self, elapsed: Duration) {
        self.scene_query_latency_ms
            .fetch_add(duration_millis(elapsed), Ordering::Relaxed);
    }

    /// 记录 scene merge 的端到端耗时。
    pub fn record_scene_merge_latency(&self, elapsed: Duration) {
        self.scene_merge_latency_ms
            .fetch_add(duration_millis(elapsed), Ordering::Relaxed);
    }

    /// 读取当前指标快照。
    pub fn snapshot(&self) -> VisionMetricsSnapshot {
        VisionMetricsSnapshot {
            capture_frames: self.capture_frames.load(Ordering::Relaxed),
            captured_pixels: self.captured_pixels.load(Ordering::Relaxed),
            ocr_processed_pixels: self.ocr_processed_pixels.load(Ordering::Relaxed),
            tiny_requests: self.tiny_requests.load(Ordering::Relaxed),
            medium_requests: self.medium_requests.load(Ordering::Relaxed),
            cancelled_stale_requests: self.cancelled_stale_requests.load(Ordering::Relaxed),
            scenes_built: self.scenes_built.load(Ordering::Relaxed),
            scene_queries: self.scene_queries.load(Ordering::Relaxed),
            last_changed_area_ratio_ppm: self.last_changed_area_ratio_ppm.load(Ordering::Relaxed),
            diff_observations: self.diff_observations.load(Ordering::Relaxed),
            stable_frame_latency_ms: self.stable_frame_latency_ms.load(Ordering::Relaxed),
            tiny_latency_ms: self.tiny_latency_ms.load(Ordering::Relaxed),
            medium_latency_ms: self.medium_latency_ms.load(Ordering::Relaxed),
            scene_merge_latency_ms: self.scene_merge_latency_ms.load(Ordering::Relaxed),
            scene_query_latency_ms: self.scene_query_latency_ms.load(Ordering::Relaxed),
            max_worker_queue_depth: self.max_worker_queue_depth.load(Ordering::Relaxed),
        }
    }
}

/// 将墙钟耗时压缩为可序列化的毫秒计数。
fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

/// 可序列化的视觉指标快照。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisionMetricsSnapshot {
    /// 已接收捕获帧数。
    pub capture_frames: u64,
    /// 捕获像素累计数。
    pub captured_pixels: u64,
    /// OCR ROI 像素累计数。
    pub ocr_processed_pixels: u64,
    /// tiny 请求数。
    pub tiny_requests: u64,
    /// medium 请求数。
    pub medium_requests: u64,
    /// stale 请求取消数。
    pub cancelled_stale_requests: u64,
    /// scene 构建数。
    pub scenes_built: u64,
    /// scene query 数。
    pub scene_queries: u64,
    /// 最近一次稳定帧变化比例，按百万分之一存储。
    pub last_changed_area_ratio_ppm: u64,
    /// 已观测的稳定帧差分次数。
    pub diff_observations: u64,
    /// 稳定帧门控累计耗时，单位为毫秒。
    pub stable_frame_latency_ms: u64,
    /// tiny OCR 累计耗时，单位为毫秒。
    pub tiny_latency_ms: u64,
    /// medium OCR 累计耗时，单位为毫秒。
    pub medium_latency_ms: u64,
    /// scene merge 累计耗时，单位为毫秒。
    pub scene_merge_latency_ms: u64,
    /// scene query 累计耗时，单位为毫秒。
    pub scene_query_latency_ms: u64,
    /// 观测到的 worker 最大排队深度。
    pub max_worker_queue_depth: u64,
}

impl VisionMetricsSnapshot {
    /// 计算 OCR 处理像素与捕获像素的比例。
    pub fn ocr_pixel_ratio(self) -> f64 {
        if self.captured_pixels == 0 {
            0.0
        } else {
            self.ocr_processed_pixels as f64 / self.captured_pixels as f64
        }
    }

    /// 返回最近一次稳定帧变化比例。
    pub fn last_changed_area_ratio(self) -> f64 {
        self.last_changed_area_ratio_ppm as f64 / 1_000_000.0
    }

    /// 返回累计 OCR ROI 数量。
    pub const fn ocr_roi_count(self) -> u64 {
        self.tiny_requests + self.medium_requests
    }
}
