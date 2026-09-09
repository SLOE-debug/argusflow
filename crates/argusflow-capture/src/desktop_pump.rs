//! 原生取帧与 CPU 差分隔离；有界冻结像素队列不允许静默丢帧。
use argusflow_core::{
    CaptureFailure,
    capture::{ScreenCaptureSource, ScreenCaptureUpdate},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc::{self, Receiver, SyncSender},
};

/// 即使下游线程创建失败，已启动的原生线程也必须停止并回收。
pub(super) struct PumpThread {
    stop: Arc<std::sync::atomic::AtomicBool>,
    scheduler: Arc<dyn crate::CaptureScheduler>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl PumpThread {
    pub fn new(
        stop: Arc<std::sync::atomic::AtomicBool>,
        scheduler: Arc<dyn crate::CaptureScheduler>,
        thread: std::thread::JoinHandle<()>,
    ) -> Self {
        Self {
            stop,
            scheduler,
            thread: Some(thread),
        }
    }
}
impl Drop for PumpThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.scheduler.wake();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// 在消费完成之前保持内存预留，包含当前正在差分的一批。
pub(super) struct FrozenBatch {
    pub updates: Vec<ScreenCaptureUpdate>,
    pub drained: bool,
    bytes: usize,
    reserved: Arc<AtomicUsize>,
}
impl Drop for FrozenBatch {
    fn drop(&mut self) {
        self.reserved.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

pub(super) struct FramePump {
    sender: SyncSender<Result<FrozenBatch, CaptureFailure>>,
    reserved: Arc<AtomicUsize>,
}
impl FramePump {
    pub fn channel() -> (Self, Receiver<Result<FrozenBatch, CaptureFailure>>) {
        let (sender, receiver) = mpsc::sync_channel(8);
        (
            Self {
                sender,
                reserved: Arc::new(AtomicUsize::new(0)),
            },
            receiver,
        )
    }
    /// 返回 false 表示队列已关闭；过载明确上报，不能阻塞原生采集。
    pub fn poll(
        &self,
        source: &dyn ScreenCaptureSource,
        draining: bool,
    ) -> Result<(), CaptureFailure> {
        let result = if draining {
            source.drain()
        } else {
            source.poll().map(|frames| (frames, true))
        };
        let batch = match result {
            Ok((updates, pending)) => {
                if updates.is_empty() && (!draining || pending) {
                    return Ok(());
                }
                let bytes = updates
                    .iter()
                    .flat_map(|update| &update.patches)
                    .map(|patch| patch.pixels().len())
                    .sum::<usize>();
                self.reserved
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                        current
                            .checked_add(bytes)
                            .filter(|total| *total <= 128 * 1024 * 1024)
                    })
                    .map_err(|_| CaptureFailure::Capacity)?;
                Ok(FrozenBatch {
                    updates,
                    drained: draining && !pending,
                    bytes,
                    reserved: self.reserved.clone(),
                })
            }
            Err(_) => Err(CaptureFailure::Unavailable),
        };
        self.sender.try_send(batch).map_err(|error| match error {
            mpsc::TrySendError::Full(_) => CaptureFailure::Capacity,
            mpsc::TrySendError::Disconnected(_) => CaptureFailure::Closed,
        })
    }
}
