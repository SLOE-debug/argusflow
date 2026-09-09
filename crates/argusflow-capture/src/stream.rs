//! 有界、多消费者变化日志；OCR 游标合并变化，录制游标逐次交付。

use crate::{FrameSnapshot, PixelRect};
use argusflow_core::{
    CaptureDelivery, CaptureFailure, CaptureGeneration, CaptureRevision, CaptureSourceId,
    CaptureTiming,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

/// 一次不可变变化的完整身份与像素快照。
#[derive(Clone)]
pub struct FrameUpdate {
    /// 唯一来源；不同来源不得交叉应用变化。
    pub source: CaptureSourceId,
    /// 来源重建后递增的拓扑代数。
    pub generation: CaptureGeneration,
    /// 来源内单调增加的变化序号。
    pub revision: CaptureRevision,
    /// 呈现和像素冻结时刻。
    pub timing: CaptureTiming,
    /// 与上一版本相比的精确变化区域。
    pub changes: Arc<[PixelRect]>,
    /// 不可变分块图像。
    pub snapshot: Arc<FrameSnapshot>,
}

/// 每个消费者独立获得的帧和累计失效范围。
pub struct FrameDelivery {
    /// 有序模式的一帧或最新模式的最新帧。
    pub frame: Arc<FrameUpdate>,
    /// 包括所有跳过的版本，不只包含最新帧的变化。
    pub changes: Vec<PixelRect>,
    /// 历史不足或拓扑改变，需要重新建立消费者完整基准。
    pub reset: bool,
}

struct StreamState {
    frames: VecDeque<Arc<FrameUpdate>>,
    bytes: usize,
    closed: bool,
}

/// 多订阅者共享的有限变化历史，发布不等待任何消费者。
pub struct CaptureStream {
    source: CaptureSourceId,
    max_frames: usize,
    max_bytes: std::sync::atomic::AtomicUsize,
    state: Mutex<StreamState>,
}

impl CaptureStream {
    /// 建立有明确项数和像素预算的来源日志。
    pub fn new(
        source: CaptureSourceId,
        max_frames: usize,
        max_bytes: usize,
    ) -> Result<Arc<Self>, CaptureFailure> {
        if max_frames == 0 || max_bytes == 0 {
            return Err(CaptureFailure::Capacity);
        }
        Ok(Arc::new(Self {
            source,
            max_frames,
            max_bytes: std::sync::atomic::AtomicUsize::new(max_bytes),
            state: Mutex::new(StreamState {
                frames: VecDeque::new(),
                bytes: 0,
                closed: false,
            }),
        }))
    }

    /// 发布源内严格递增的变化；历史淘汰不阻塞其他订阅。
    pub fn publish(&self, frame: FrameUpdate) -> Result<(), CaptureFailure> {
        let mut state = self.state.lock().map_err(|_| CaptureFailure::Unavailable)?;
        if state.closed {
            return Err(CaptureFailure::Closed);
        }
        if frame.source != self.source
            || state
                .frames
                .back()
                .is_some_and(|last| frame.revision <= last.revision)
        {
            return Err(CaptureFailure::Unavailable);
        }
        let bytes = frame.snapshot.byte_len();
        let max_bytes = self.max_bytes.load(std::sync::atomic::Ordering::Acquire);
        if bytes > max_bytes {
            return Err(CaptureFailure::Capacity);
        }
        // 保守按完整像素预算计费，确保即使所有块均改变也不会超额。
        while state.frames.len() >= self.max_frames || state.bytes + bytes > max_bytes {
            if let Some(old) = state.frames.pop_front() {
                state.bytes -= old.snapshot.byte_len();
            }
        }
        state.bytes += bytes;
        state.frames.push_back(Arc::new(frame));
        Ok(())
    }

    /// 创建独立消费游标；首次交付必定标记完整基准。
    pub(crate) fn set_budget(&self, bytes: usize) -> Result<(), CaptureFailure> {
        let mut state = self.state.lock().map_err(|_| CaptureFailure::Unavailable)?;
        if state
            .frames
            .back()
            .is_some_and(|frame| frame.snapshot.byte_len() > bytes)
        {
            return Err(CaptureFailure::Capacity);
        }
        self.max_bytes
            .store(bytes, std::sync::atomic::Ordering::Release);
        while state.bytes > bytes {
            if let Some(frame) = state.frames.pop_front() {
                state.bytes -= frame.snapshot.byte_len();
            } else {
                break;
            }
        }
        Ok(())
    }

    /// 创建独立消费游标；首次交付必定标记完整基准。
    pub fn subscribe(self: &Arc<Self>, delivery: CaptureDelivery) -> CaptureCursor {
        CaptureCursor {
            stream: self.clone(),
            delivery,
            previous: None,
        }
    }

    /// 返回最新已冻结快照，不消费其他订阅。
    pub fn subscribe_snapshot(
        self: &Arc<Self>,
        delivery: CaptureDelivery,
    ) -> Result<(CaptureCursor, Arc<FrameUpdate>), CaptureFailure> {
        let state = self.state.lock().map_err(|_| CaptureFailure::Unavailable)?;
        let frame = state
            .frames
            .back()
            .cloned()
            .ok_or(CaptureFailure::Unavailable)?;
        let cursor = CaptureCursor {
            stream: self.clone(),
            delivery,
            previous: Some((frame.revision, frame.generation)),
        };
        Ok((cursor, frame))
    }

    /// 返回最新已冻结快照，不消费其他订阅。
    pub fn latest(&self) -> Result<Option<Arc<FrameUpdate>>, CaptureFailure> {
        Ok(self
            .state
            .lock()
            .map_err(|_| CaptureFailure::Unavailable)?
            .frames
            .back()
            .cloned())
    }

    /// 停止来源；已有变化仍可排空。
    pub fn close(&self) -> Result<(), CaptureFailure> {
        self.state
            .lock()
            .map_err(|_| CaptureFailure::Unavailable)?
            .closed = true;
        Ok(())
    }
}

/// 由一个消费者独占的游标，不能任意覆盖源的历史或状态。
pub struct CaptureCursor {
    stream: Arc<CaptureStream>,
    delivery: CaptureDelivery,
    previous: Option<(CaptureRevision, CaptureGeneration)>,
}

impl CaptureCursor {
    /// 非阻塞获取可交付变化；有序历史缺失报错，最新消费者显式完整失效。
    pub fn next(&mut self) -> Result<Option<FrameDelivery>, CaptureFailure> {
        let state = self
            .stream
            .state
            .lock()
            .map_err(|_| CaptureFailure::Unavailable)?;
        let missed = self.previous.is_some_and(|(revision, _)| {
            state
                .frames
                .front()
                .is_some_and(|first| first.revision.0 > revision.0 + 1)
        });
        if missed && self.delivery == CaptureDelivery::Ordered {
            return Err(CaptureFailure::HistoryGap);
        }
        let pending = state
            .frames
            .iter()
            .filter(|frame| {
                self.previous
                    .is_none_or(|(revision, _)| frame.revision > revision)
            })
            .collect::<Vec<_>>();
        let frame = match self.delivery {
            CaptureDelivery::Ordered => pending.first(),
            CaptureDelivery::Latest => pending.last(),
        };
        let Some(frame) = frame else {
            return if state.closed {
                Err(CaptureFailure::Closed)
            } else {
                Ok(None)
            };
        };
        let frame = Arc::clone(frame);
        let reset = missed
            || self
                .previous
                .is_none_or(|(_, generation)| generation != frame.generation)
            || pending
                .iter()
                .any(|entry| entry.generation != frame.generation);
        let changes = if reset {
            frame
                .snapshot
                .blocks()
                .iter()
                .map(|block| block.bounds())
                .collect()
        } else if self.delivery == CaptureDelivery::Ordered {
            frame.changes.to_vec()
        } else {
            let mut changes = pending
                .iter()
                .flat_map(|frame| frame.changes.iter().copied())
                .collect::<Vec<_>>();
            changes.sort_by_key(|rect| (rect.y, rect.x, rect.height, rect.width));
            changes.dedup();
            changes
        };
        self.previous = Some((frame.revision, frame.generation));
        Ok(Some(FrameDelivery {
            frame,
            changes,
            reset,
        }))
    }
}
