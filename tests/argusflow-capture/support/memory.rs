//! 只存在于根测试目录的确定性来源，支持版本、时间、水位和故障注入。
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
pub const SOURCE: SourceId = SourceId(7);
pub fn time(ms: u64) -> ClockTime {
    ClockTime(ms * 1_000_000)
}
pub struct Memory {
    events: Mutex<Vec<BackendEvent>>,
    pub now: AtomicU64,
    pub reads: Arc<AtomicU64>,
    pub valid: Arc<AtomicBool>,
    budget: ByteBudget,
}
impl Memory {
    pub fn pending(&self) -> bool {
        !self.events.lock().unwrap().is_empty()
    }
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            events: Mutex::new(Vec::new()),
            now: AtomicU64::new(0),
            reads: Arc::new(AtomicU64::new(0)),
            valid: Arc::new(AtomicBool::new(true)),
            budget: ByteBudget::new(1024 * 1024).unwrap(),
        })
    }
    pub fn emit(&self, event: BackendEvent) {
        self.events.lock().unwrap().push(event);
    }
    pub fn setup(&self) {
        self.emit(BackendEvent::Source(SourceInfo {
            id: SOURCE,
            name: "test".into(),
            generation: 1,
            bounds: ScreenRect::new(-16, -8, 16, 16).unwrap(),
            rotation: Rotation::Identity,
            dpi: (96, 96),
            state: SourceState::Ready,
            failure: None,
        }));
        self.emit(BackendEvent::Baseline(self.frame(0, 1, vec![0; 1024])));
    }
    pub fn frame(&self, revision: u64, ms: u64, bytes: Vec<u8>) -> Arc<Snapshot> {
        Arc::new(Snapshot {
            version: Version {
                session: 1,
                source: SOURCE,
                generation: 1,
                revision,
            },
            bounds: ScreenRect::new(-16, -8, 16, 16).unwrap(),
            timing: Timing {
                presented: Some(time(ms)),
                acquired: time(ms),
                frozen: time(ms),
            },
            pixels: Arc::new(Pixels {
                bytes,
                reads: self.reads.clone(),
                valid: self.valid.clone(),
                budget: self.budget.clone(),
            }),
        })
    }
    pub fn changed(&self, revision: u64, ms: u64, bytes: Vec<u8>, region: PixelRect) {
        self.emit(BackendEvent::Changed {
            snapshot: self.frame(revision, ms, bytes),
            changes: PixelChanges {
                regions: vec![region],
                compared_pixels: 1,
                changed_pixels: 1,
            },
        });
    }
    pub fn watermark(&self, ms: u64) {
        self.now.store(time(ms).0, Ordering::Release);
        self.emit(BackendEvent::Watermark {
            source: SOURCE,
            generation: 1,
            through: time(ms),
        });
    }
}
impl DesktopBackend for Memory {
    fn claim_consumer(&self) -> CaptureResult<()> {
        Ok(())
    }
    fn release_consumer(&self) {}
    fn clock(&self) -> ClockDomain {
        ClockDomain {
            session: 1,
            origin: 0,
            frequency: 1_000_000_000,
        }
    }
    fn now(&self) -> ClockTime {
        ClockTime(self.now.load(Ordering::Acquire))
    }
    fn poll(&self) -> CaptureResult<Vec<BackendEvent>> {
        Ok(std::mem::take(&mut *self.events.lock().unwrap()))
    }
    fn stats(&self) -> CaptureStats {
        CaptureStats {
            pixel_readback_bytes: self.reads.load(Ordering::Acquire),
            ..Default::default()
        }
    }
    fn restart(&self) -> CaptureResult<()> {
        Ok(())
    }
    fn shutdown(&self, _: Operation) -> CaptureFuture<()> {
        Box::pin(async { Ok(()) })
    }
}
struct Pixels {
    bytes: Vec<u8>,
    reads: Arc<AtomicU64>,
    valid: Arc<AtomicBool>,
    budget: ByteBudget,
}
impl SnapshotPixels for Pixels {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    fn read(self: Arc<Self>, region: PixelRect, operation: Operation) -> CaptureFuture<PixelImage> {
        Box::pin(async move {
            operation.check("test_read")?;
            if !self.valid() {
                return Err(CaptureError::new(
                    FailureKind::StaleHandle,
                    "test_read",
                    "revoked",
                ));
            }
            let size = region.byte_len() as usize;
            let reservation = self.budget.reserve(size)?;
            let mut bytes = vec![0; size];
            for y in 0..region.height() {
                let source = ((region.y() + y) * 16 + region.x()) as usize * 4;
                let dest = (y * region.width()) as usize * 4;
                let count = region.width() as usize * 4;
                bytes[dest..dest + count].copy_from_slice(&self.bytes[source..source + count]);
            }
            self.reads.fetch_add(size as u64, Ordering::Relaxed);
            PixelImage::new(
                region.width(),
                region.height(),
                region.width() as usize * 4,
                PixelFormat::Bgrx8,
                bytes,
                reservation,
            )
        })
    }
    fn compare(
        self: Arc<Self>,
        before: Arc<dyn SnapshotPixels>,
        regions: Vec<PixelRect>,
        operation: Operation,
    ) -> CaptureFuture<PixelChanges> {
        Box::pin(async move {
            operation.check("test_compare")?;
            let before = before.as_any().downcast_ref::<Pixels>().unwrap();
            let mut output = PixelChanges::default();
            for r in regions {
                for y in r.y()..r.bottom() {
                    for x in r.x()..r.right() {
                        let i = (y * 16 + x) as usize * 4;
                        output.compared_pixels += 1;
                        if self.bytes[i..i + 3] != before.bytes[i..i + 3] {
                            output.changed_pixels += 1;
                            output.regions.push(PixelRect::new(x, y, 1, 1)?);
                        }
                    }
                }
            }
            Ok(output)
        })
    }
}
