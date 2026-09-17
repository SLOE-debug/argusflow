//! 来源重建、停止或屏幕几何变化后拒绝使用旧 OCR 坐标。
use argusflow_capture_contracts::*;
use std::sync::Arc;

pub(crate) struct Validity {
    pub source: Arc<dyn DesktopFrameSource>,
    pub version: Version,
    pub bounds: ScreenRect,
}
impl SourceValidity for Validity {
    fn valid(&self) -> bool {
        if self.source.clock().session != self.version.session {
            return false;
        }
        self.source.history().is_ok_and(|histories| {
            histories.iter().any(|history| {
                history.source.id == self.version.source
                    && history.source.generation == self.version.generation
                    && history.source.bounds == self.bounds
                    && history.source.state == SourceState::Ready
                    && history.source.failure.is_none()
            })
        })
    }
}
