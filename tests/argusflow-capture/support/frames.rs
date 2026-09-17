//! 可控制水位、代际和像素的帧源，不采集用户桌面。
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
pub struct Frames {
    pub history: Mutex<FrameHistory>,
    pub stopped: AtomicBool,
}
pub fn frame(revision: u64, millis: u64, changed: Option<(usize, u8)>) -> Arc<DesktopFrame> {
    let mut bytes = vec![0; 8 * 4 * 4];
    if let Some((pixel, value)) = changed {
        bytes[pixel * 4] = value;
    }
    let budget = ByteBudget::new(bytes.len()).unwrap();
    let image = PixelImage::new(
        8,
        4,
        32,
        PixelFormat::Bgrx8,
        bytes,
        budget.reserve(128).unwrap(),
    )
    .unwrap();
    let time = ClockTime(millis * 1_000_000);
    Arc::new(DesktopFrame {
        version: Version {
            session: 1,
            source: SourceId(1),
            generation: 1,
            revision,
        },
        source: SourceInfo {
            id: SourceId(1),
            name: "test".into(),
            generation: 1,
            bounds: ScreenRect::new(-100, 20, 8, 4).unwrap(),
            rotation: Rotation::Identity,
            dpi: (96, 96),
            state: SourceState::Ready,
            failure: None,
        },
        timing: Timing {
            presented: Some(time),
            acquired: time,
            frozen: time,
        },
        image,
    })
}
impl Frames {
    pub fn new() -> Arc<Self> {
        let first = frame(1, 0, None);
        Arc::new(Self {
            history: Mutex::new(FrameHistory {
                source: first.source.clone(),
                checked: ClockTime(1_000_000_000),
                frames: vec![first],
            }),
            stopped: AtomicBool::new(false),
        })
    }
}
impl DesktopFrameSource for Frames {
    fn clock(&self) -> ClockDomain {
        ClockDomain {
            session: 1,
            origin: 0,
            frequency: 1000,
        }
    }
    fn now(&self) -> ClockTime {
        ClockTime(1_000_000_000)
    }
    fn history(&self) -> CaptureResult<Vec<FrameHistory>> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(CaptureError::new(FailureKind::Closed, "test", "stopped"));
        }
        Ok(vec![self.history.lock().unwrap().clone()])
    }
    fn shutdown(&self, _: Operation) -> CaptureFuture<()> {
        self.stopped.store(true, Ordering::Release);
        Box::pin(async { Ok(()) })
    }
}
