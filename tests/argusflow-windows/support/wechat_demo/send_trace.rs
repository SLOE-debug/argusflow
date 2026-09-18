//! 发送瞬间的有界连续采样；采样热路径不调用 OCR，也不保存截图历史。
use super::{
    bubbles, capture,
    desktop::Desktop,
    frame::Frame,
    message_tracking::{Tracker, Uncertainty},
};
use argusflow_capture_contracts::PixelRect;
use argusflow_core::Key;
use std::{
    error::Error,
    time::{Duration, Instant},
};

/// 对应本次 Enter 的轨迹证据，最多持有最终一张窗口截图。
pub struct Trace {
    /// 最后采集的原图，后续 OCR 和几何必须使用同一帧。
    pub frame: Frame,
    /// 未确认不是失败，也不允许由此触发重发。
    pub candidate: Result<PixelRect, Uncertainty>,
}

/// 基线必须为刚采集、已确认草稿的窗口；最多采样 2 秒/80 帧。
/// Enter 与采样通过 join 并发驱动，两条分支都结束后才释放资源。
pub async fn send_once(
    desktop: &Desktop,
    baseline: Frame,
    region: PixelRect,
) -> Result<Trace, Box<dyn Error>> {
    let initial = bubbles::outgoing(&baseline, region);
    let initial_count = initial.len();
    let mut previous_boxes = initial.clone();
    let mut tracker = Tracker::new(region, initial);
    let bounds = baseline.bounds;
    let capture_loop = async {
        let started = Instant::now();
        let mut previous = started;
        let mut last = baseline;
        let mut count = 0;
        let mut max_gap = Duration::ZERO;
        let mut tracking_time = Duration::ZERO;
        while count < 80 && started.elapsed() < Duration::from_secs(2) {
            let bytes = capture::capture(desktop, bounds, bounds).await?;
            let frame = Frame::from_bgrx(bounds, bytes)?;
            let now = Instant::now();
            let gap = now.duration_since(previous);
            previous = now;
            max_gap = max_gap.max(gap);
            let tracking_started = Instant::now();
            let boxes = bubbles::outgoing(&frame, region);
            let shift =
                super::pixel_motion::estimate(&last, &frame, region, &previous_boxes, &boxes);
            tracker.observe(boxes.clone(), gap, shift);
            previous_boxes = boxes;
            tracking_time += tracking_started.elapsed();
            last = frame;
            count += 1;
            tokio::time::sleep(Duration::from_millis(12)).await;
        }
        let candidate = tracker.candidate();
        let final_count = bubbles::outgoing(&last, region).len();
        println!("可见发出气泡数 {initial_count}→{final_count}（仅诊断，不作为判定依据）");
        println!(
            "发送跟踪：{count} 帧，最大间隔 {}ms，检测/关联合计 {}ms，证据 {:?}",
            max_gap.as_millis(),
            tracking_time.as_millis(),
            candidate
        );
        Ok::<_, Box<dyn Error>>(Trace {
            frame: last,
            candidate,
        })
    };
    let (key, trace) = tokio::join!(desktop.key(Key::Enter), capture_loop);
    // 输入服务报错也可能已经产生副作用；保留“未确认”语义而不是自动重放。
    key.map_err(|error| format!("Enter 结果未确认，不重发：{error}"))?;
    trace
}
