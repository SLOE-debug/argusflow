//! 桌面主机保持存活，录制订阅退出不影响最新帧消费者。
use super::*;
use argusflow_core::*;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

struct Source {
    next: AtomicU64,
}

#[test]
fn frozen_queue_is_bounded_ordered_and_reusable_after_overload() {
    let (pump, receiver) = crate::desktop_pump::FramePump::channel();
    let source = Source {
        next: AtomicU64::new(0),
    };
    for _ in 0..8 {
        pump.poll(&source, false).unwrap();
    }
    assert_eq!(pump.poll(&source, false), Err(CaptureFailure::Capacity));
    for expected in 0..8 {
        let batch = receiver.try_recv().unwrap().unwrap();
        assert_eq!(batch.updates[0].timing.presented_us, expected);
    }
    pump.poll(&source, false).unwrap();
    assert_eq!(
        receiver.try_recv().unwrap().unwrap().updates[0]
            .timing
            .presented_us,
        9
    );
    pump.poll(&source, true).unwrap();
    assert!(receiver.try_recv().unwrap().unwrap().drained);
    drop(receiver);
    assert_eq!(pump.poll(&source, false), Err(CaptureFailure::Closed));
}
impl capture::ScreenCaptureSource for Source {
    fn sources(&self) -> Result<Vec<CaptureSourceId>, CaptureError> {
        Ok(vec![CaptureSourceId(1)])
    }
    fn clock_us(&self) -> Result<u64, CaptureError> {
        Ok(0)
    }
    fn drain(&self) -> Result<(Vec<capture::ScreenCaptureUpdate>, bool), CaptureError> {
        Ok((Vec::new(), false))
    }
    fn poll(&self) -> Result<Vec<capture::ScreenCaptureUpdate>, CaptureError> {
        let revision = self.next.fetch_add(1, Ordering::Relaxed);
        let bounds = InspectionRect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        Ok(vec![capture::ScreenCaptureUpdate {
            source: CaptureSourceId(1),
            generation: CaptureGeneration(1),
            bounds,
            timing: CaptureTiming {
                presented_us: revision,
                frozen_us: revision,
            },
            reset: revision == 0,
            accumulated_frames: 1,
            patches: vec![
                EvidenceFrame::new(
                    bounds,
                    1,
                    1,
                    EvidencePixelFormat::Rgba8,
                    vec![revision as u8, 0, 0, 255],
                )
                .unwrap(),
            ],
        }])
    }
}
struct Scheduler {
    thread: Mutex<Option<std::thread::Thread>>,
}
impl CaptureScheduler for Scheduler {
    fn wait(&self) -> Result<(), CaptureFailure> {
        *self.thread.lock().unwrap() = Some(std::thread::current());
        std::thread::park_timeout(Duration::from_millis(1));
        Ok(())
    }
    fn wake(&self) {
        if let Some(thread) = self.thread.lock().unwrap().as_ref() {
            thread.unpark();
        }
    }
}
#[test]
fn stopping_one_subscriber_preserves_native_host_and_other_cursor() {
    let hub = DesktopCaptureHub::start(
        Arc::new(Source {
            next: AtomicU64::new(0),
        }),
        Arc::new(Scheduler {
            thread: Mutex::new(None),
        }),
    )
    .unwrap();
    let mut recorder = hub.subscribe(CaptureDelivery::Ordered).unwrap();
    let mut ocr = hub.subscribe(CaptureDelivery::Latest).unwrap();
    assert!(!recorder.poll().unwrap().is_empty());
    let old = ocr.poll().unwrap().last().unwrap().revision;
    drop(recorder);
    let cutoff = hub
        .request_barrier()
        .unwrap()
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
        .unwrap();
    assert!(cutoff.contains_key(&CaptureSourceId(1)));
    // 两个独立线程的交付不保证在固定 10ms 内完成；等待实际版本推进并保留失败期限。
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        if ocr
            .poll()
            .unwrap()
            .last()
            .is_some_and(|frame| frame.revision > old)
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "capture did not resume after barrier"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut another = hub.subscribe(CaptureDelivery::Ordered).unwrap();
    assert!(another.poll().unwrap().first().unwrap().revision > old);
    drop(another);
    drop(ocr);
    hub.shutdown().unwrap();
}
