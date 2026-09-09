//! mutable 桌面仅由采集线程持有，交付的 TileMap 从不修改。
use crate::capture::{
    clock::Clock,
    gpu::{Completion, Graphics, PendingDifference, Texture, TileBatch, TileMap, failure},
    lease::Epoch,
    queue::EventQueue,
    requests::Request,
    topology::OutputInfo,
};
use argusflow_capture_contracts::*;
use std::sync::{Arc, mpsc::SyncSender};
use windows::Win32::Graphics::Dxgi::IDXGIOutputDuplication;

pub(in crate::capture) struct PendingFrame {
    pub batches: Vec<(TileBatch, PendingDifference)>,
    pub timing: Timing,
    pub changes: PixelChanges,
    pub next: TileMap,
    pub index: usize,
    pub completion: Option<Completion>,
}
pub(crate) struct Output {
    pub info: SourceInfo,
    pub width: u32,
    pub height: u32,
    pub duplication: IDXGIOutputDuplication,
    pub desktop: Option<Arc<Texture>>,
    pub map: Option<TileMap>,
    pub pending: Option<PendingFrame>,
    pub baseline: Option<(Completion, Timing)>,
    pub valid: Arc<Epoch>,
    pub revision: u64,
    pub last_present: ClockTime,
    pub checked: ClockTime,
    pub clock: Clock,
    pub sender: SyncSender<Request>,
    pub queue: Arc<EventQueue>,
    pub pressure: bool,
}
impl Output {
    pub fn new(
        spec: OutputInfo,
        graphics: &Graphics,
        clock: Clock,
        sender: SyncSender<Request>,
        queue: Arc<EventQueue>,
    ) -> CaptureResult<Self> {
        // SAFETY: spec.output 与 graphics.device 在同一适配器上枚举和创建。
        let duplication =
            unsafe { spec.output.DuplicateOutput(&graphics.device) }.map_err(failure)?;
        Ok(Self {
            info: spec.info,
            width: spec.raw_width,
            height: spec.raw_height,
            duplication,
            desktop: None,
            map: None,
            pending: None,
            baseline: None,
            valid: Arc::new(Epoch::new()),
            revision: 0,
            last_present: ClockTime(0),
            checked: ClockTime(0),
            clock,
            sender,
            queue,
            pressure: false,
        })
    }
    pub fn gap(&self, reason: GapReason) {
        self.queue.push(
            BackendEvent::Gap(Gap {
                source: self.info.id,
                from: self.last_present,
                through: self.clock.now(),
                reason,
            }),
            self.clock.now(),
        );
    }
    pub fn revoke(&mut self) {
        self.valid.revoke();
        self.map = None;
        self.pending = None;
        self.baseline = None;
    }
    pub fn pressure(&mut self) {
        if !self.pressure {
            self.gap(GapReason::Capacity);
            self.map = None;
            self.pending = None;
            self.baseline = None;
            self.info.generation += 1;
            self.revision = 0;
            // 预算压力释放自己的普通索引；消费者已固定的像素仍然可读且持续计费。
            self.pressure = true;
            self.info.state = SourceState::Recovering;
            self.queue
                .push(BackendEvent::Source(self.info.clone()), self.clock.now());
        }
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        self.valid.revoke();
    }
}
