//! 用固定测试图片验证采样 OCR 装配，不采集用户桌面。
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
pub struct FixtureSource {
    pub image: PixelImage,
    pub calls: AtomicU64,
    pub revision: AtomicU64,
    pub valid: Arc<AtomicBool>,
}
impl FixtureSource {
    pub fn new(image: PixelImage) -> Arc<Self> {
        Arc::new(Self {
            image,
            calls: AtomicU64::new(0),
            revision: AtomicU64::new(0),
            valid: Arc::new(AtomicBool::new(true)),
        })
    }
}
impl RegionSource for FixtureSource {
    fn sample(&self, request: SampleRequest, operation: Operation) -> CaptureFuture<RegionSample> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let image = self.image.clone();
        let valid = self.valid.clone();
        let revision = self.revision.load(Ordering::Acquire);
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            operation.check("fixture_sample")?;
            let version = Version {
                session: 1,
                source: request.source,
                generation: 1,
                revision,
            };
            let time = ClockTime((revision + 1) * 1_000_000);
            let snapshot = Arc::new(Snapshot {
                version,
                bounds: ScreenRect::new(-100, 20, image.width(), image.height())?,
                timing: Timing {
                    presented: Some(time),
                    acquired: time,
                    frozen: time,
                },
                pixels: Arc::new(FixturePixels(valid)),
            });
            let content = if request.previous.is_some() {
                SampleContent::Unchanged
            } else {
                SampleContent::Image(image)
            };
            Ok(RegionSample {
                token: ContentToken {
                    snapshot,
                    region: request.region,
                },
                content,
                observed_version: version,
                observed_through: time,
            })
        })
    }
}
struct FixturePixels(Arc<AtomicBool>);
impl SnapshotPixels for FixturePixels {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn valid(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    fn read(self: Arc<Self>, _: PixelRect, _: Operation) -> CaptureFuture<PixelImage> {
        Box::pin(async {
            Err(CaptureError::new(
                FailureKind::Unsupported,
                "fixture",
                "fixture already supplies image",
            ))
        })
    }
    fn compare(
        self: Arc<Self>,
        _: Arc<dyn SnapshotPixels>,
        _: Vec<PixelRect>,
        _: Operation,
    ) -> CaptureFuture<PixelChanges> {
        Box::pin(async {
            Err(CaptureError::new(
                FailureKind::Unsupported,
                "fixture",
                "fixture handles content identity",
            ))
        })
    }
}
