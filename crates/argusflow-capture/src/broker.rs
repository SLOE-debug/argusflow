//! 原生区域更新归一化为多消费者共享的不可变变化流。

use crate::{CaptureStream, FrameSnapshot, FrameUpdate};
use argusflow_core::{
    CaptureFailure, CaptureGeneration, CaptureRevision, CaptureSourceId,
    capture::ScreenCaptureUpdate,
};
use std::{collections::HashMap, sync::Arc};

struct SourceState {
    snapshot: Arc<FrameSnapshot>,
    generation: CaptureGeneration,
    revision: CaptureRevision,
    stream: Arc<CaptureStream>,
}

/// 由采集主机线程独占推进；消费者只取得只读来源句柄。
#[derive(Default)]
pub struct CaptureBroker {
    sources: HashMap<CaptureSourceId, SourceState>,
}

impl CaptureBroker {
    /// 查询已有来源，重复订阅共享同一日志与像素。
    pub fn source(&self, source: CaptureSourceId) -> Option<Arc<CaptureStream>> {
        self.sources.get(&source).map(|state| state.stream.clone())
    }

    /// 接收独立像素，精确比较后发布；重绘但未变化不会增加版本。
    pub fn publish(
        &mut self,
        update: ScreenCaptureUpdate,
    ) -> Result<Option<FrameUpdate>, CaptureFailure> {
        self.publish_update(update, true)
    }

    /// 保留每次原生呈现及候选区，包括重绘但像素相同的帧；精确化由消费者稍后执行。
    pub fn publish_candidates(
        &mut self,
        update: ScreenCaptureUpdate,
    ) -> Result<Option<FrameUpdate>, CaptureFailure> {
        self.publish_update(update, false)
    }

    fn publish_update(
        &mut self,
        update: ScreenCaptureUpdate,
        precise: bool,
    ) -> Result<Option<FrameUpdate>, CaptureFailure> {
        let source_count =
            self.sources.len() + usize::from(!self.sources.contains_key(&update.source));
        let source_budget = 128 * 1024 * 1024 / source_count.max(1);
        for source in self.sources.values() {
            source.stream.set_budget(source_budget)?;
        }
        let state = self.sources.get(&update.source);
        let reset = update.reset
            || state.is_none_or(|state| {
                state.generation != update.generation || state.snapshot.bounds() != update.bounds
            });
        let (snapshot, changes) = if reset {
            let baseline = update
                .patches
                .first()
                .filter(|frame| frame.bounds() == update.bounds)
                .ok_or(CaptureFailure::Unavailable)?;
            let snapshot = FrameSnapshot::from_frame(baseline);
            let regions = snapshot
                .blocks()
                .iter()
                .map(|block| block.bounds())
                .collect();
            (snapshot, regions)
        } else if precise {
            state
                .ok_or(CaptureFailure::Unavailable)?
                .snapshot
                .apply_patches(&update.patches)
                .map_err(|_| CaptureFailure::Unavailable)?
        } else {
            let snapshot = state
                .ok_or(CaptureFailure::Unavailable)?
                .snapshot
                .apply_candidates(&update.patches)
                .map_err(|_| CaptureFailure::Unavailable)?;
            let changes = update
                .patches
                .iter()
                .map(|patch| crate::PixelRect {
                    x: (patch.bounds().x - update.bounds.x) as u32,
                    y: (patch.bounds().y - update.bounds.y) as u32,
                    width: patch.width(),
                    height: patch.height(),
                })
                .collect();
            (snapshot, changes)
        };
        if changes.is_empty() {
            return Ok(None);
        }
        let revision = CaptureRevision(state.map_or(1, |state| state.revision.0 + 1));
        let stream = match state {
            Some(state) => state.stream.clone(),
            None => CaptureStream::new(update.source, 256, source_budget)?,
        };
        let snapshot = Arc::new(snapshot);
        let frame = FrameUpdate {
            source: update.source,
            generation: update.generation,
            revision,
            timing: update.timing,
            changes: crate::merge_regions(changes).into(),
            snapshot: snapshot.clone(),
        };
        stream.publish(frame.clone())?;
        self.sources.insert(
            update.source,
            SourceState {
                snapshot,
                generation: update.generation,
                revision,
                stream,
            },
        );
        Ok(Some(frame))
    }

    /// 来源停止后唤醒语义由主机负责，游标排空后得到 Closed。
    pub fn close(&self) -> Result<(), CaptureFailure> {
        for source in self.sources.values() {
            source.stream.close()?;
        }
        Ok(())
    }
}
