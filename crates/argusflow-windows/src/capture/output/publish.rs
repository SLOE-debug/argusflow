//! 按 GPU 提交次序发布，版本永远引用自己的冻结像素。
use super::state::Output;
use crate::capture::{
    gpu::{Completion, Graphics, invalid},
    pixels::GpuPixels,
    topology,
};
use argusflow_capture_contracts::*;
use argusflow_core::FailureKind;
use std::sync::Arc;
impl Output {
    fn snapshot(&self, timing: Timing) -> CaptureResult<Arc<Snapshot>> {
        let map = self
            .map
            .as_ref()
            .ok_or_else(|| invalid("missing snapshot map"))?;
        Ok(Arc::new(Snapshot {
            version: Version {
                session: self.clock.0.session,
                source: self.info.id,
                generation: self.info.generation,
                revision: self.revision,
            },
            bounds: self.info.bounds,
            timing,
            pixels: Arc::new(GpuPixels {
                map: self.valid.pin(map.clone()),
                rotation: self.info.rotation,
                valid: self.valid.clone(),
                identity: (self.info.id, self.info.generation),
                sender: self.sender.clone(),
            }),
        }))
    }
    pub(super) fn publish_baseline(&self, timing: Timing) -> CaptureResult<()> {
        self.queue.push(
            BackendEvent::Baseline(self.snapshot(timing)?),
            self.clock.now(),
        );
        Ok(())
    }
    pub(super) fn watermark(&self) {
        if self.map.is_some() && self.pending.is_none() && self.baseline.is_none() {
            self.queue.push(
                BackendEvent::Watermark {
                    source: self.info.id,
                    generation: self.info.generation,
                    through: self.checked,
                },
                self.clock.now(),
            );
        }
    }
    pub fn finish(
        &mut self,
        graphics: &Graphics,
        config: &BackendConfig,
        stats: &mut CaptureStats,
    ) -> CaptureResult<()> {
        if let Some((completion, _)) = &self.baseline {
            if completion.started.elapsed() > config.gpu_timeout {
                return Err(CaptureError::new(
                    FailureKind::Timeout,
                    "baseline",
                    "GPU 基线建立超时",
                ));
            }
            if !completion.ready(graphics)? {
                return Ok(());
            }
            if let Some((_, mut timing)) = self.baseline.take() {
                timing.frozen = self.clock.now();
                self.info.state = SourceState::Ready;
                self.queue
                    .push(BackendEvent::Source(self.info.clone()), self.clock.now());
                self.publish_baseline(timing)?;
            }
        }
        let Some(frame) = self.pending.as_mut() else {
            return Ok(());
        };
        while frame.index < frame.batches.len() {
            let (batch, pending) = &frame.batches[frame.index];
            if pending.started.elapsed() > config.gpu_timeout {
                return Err(CaptureError::new(
                    FailureKind::Timeout,
                    "desktop_diff",
                    "桌面 GPU 差分超时",
                ));
            }
            let Some((changes, changed)) = pending.poll(graphics)? else {
                return Ok(());
            };
            stats.metadata_readback_bytes += pending.bytes();
            batch.apply_compact(graphics, &mut frame.next, &changed)?;
            frame.changes.changed_pixels += changes.changed_pixels;
            frame.changes.compared_pixels += changes.compared_pixels;
            for region in changes.regions {
                frame.changes.regions.push(topology::to_logical(
                    region,
                    self.width,
                    self.height,
                    self.info.rotation,
                )?);
            }
            frame.index += 1;
        }
        if frame.changes.changed_pixels != 0 {
            if frame.completion.is_none() {
                frame.completion = Some(Completion::submit(graphics)?);
                unsafe { graphics.context.Flush() };
                return Ok(());
            }
            if let Some(completion) = &frame.completion {
                if completion.started.elapsed() > config.gpu_timeout {
                    return Err(CaptureError::new(
                        FailureKind::Timeout,
                        "history_freeze",
                        "GPU 历史冻结超时",
                    ));
                }
                if !completion.ready(graphics)? {
                    return Ok(());
                }
            }
        }
        let Some(mut frame) = self.pending.take() else {
            return Ok(());
        };
        if frame.changes.changed_pixels != 0 {
            self.revision = self
                .revision
                .checked_add(1)
                .ok_or_else(|| invalid("revision overflow"))?;
            self.map = Some(frame.next);
            frame.timing.frozen = self.clock.now();
            stats.changed_frames += 1;
            let snapshot = self.snapshot(frame.timing)?;
            self.queue.push(
                BackendEvent::Changed {
                    snapshot,
                    changes: frame.changes,
                },
                self.clock.now(),
            );
        }
        self.watermark();
        Ok(())
    }
}
