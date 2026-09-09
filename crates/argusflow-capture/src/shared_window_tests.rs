//! 验证原生会话复用、独立游标及跳帧后变化累计。
use crate::*;
use argusflow_core::WindowIdentity;
use async_trait::async_trait;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[derive(Debug)]
struct Source {
    opens: AtomicUsize,
    subscription: Arc<Subscription>,
}
#[derive(Debug)]
struct Subscription {
    receiver: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Arc<CapturedFrame>>>,
}
fn window() -> WindowIdentity {
    WindowIdentity {
        handle: 1,
        process_id: 1,
    }
}
#[async_trait]
impl WindowFrameSource for Source {
    fn health(&self) -> CaptureHealth {
        CaptureHealth::new(CaptureLifecycle::Ready)
    }
    async fn open(
        &self,
        _: WindowIdentity,
        _: CapturePolicy,
    ) -> Result<Arc<dyn FrameSubscription>, CaptureError> {
        self.opens.fetch_add(1, Ordering::Relaxed);
        Ok(self.subscription.clone())
    }
}
#[async_trait]
impl FrameSubscription for Subscription {
    async fn next(&self, timeout: Duration) -> Result<Arc<CapturedFrame>, CaptureError> {
        tokio::time::timeout(timeout, self.receiver.lock().await.recv())
            .await
            .ok()
            .flatten()
            .ok_or(CaptureError::FrameTimeout { timeout_ms: 1 })
    }
    async fn current_topology_generation(&self) -> Result<TopologyGeneration, CaptureError> {
        Ok(TopologyGeneration::new(1))
    }
    fn window(&self) -> WindowIdentity {
        window()
    }
}
fn frame(id: u64, value: u8) -> Arc<CapturedFrame> {
    Arc::new(
        CapturedFrame::from_bgra8(
            FrameId::new(id),
            TopologyGeneration::new(1),
            window(),
            QpcTimestamp::new(id),
            2,
            1,
            96,
            96,
            8,
            vec![value, 0, 0, 255, 0, 0, 0, 255],
        )
        .unwrap(),
    )
}

#[tokio::test]
async fn consumers_share_native_session_and_reverted_pixels_remain_dirty() {
    let (sender, receiver) = tokio::sync::mpsc::channel(8);
    let source = Arc::new(Source {
        opens: AtomicUsize::new(0),
        subscription: Arc::new(Subscription {
            receiver: tokio::sync::Mutex::new(receiver),
        }),
    });
    let shared = SharedWindowSource::new(source.clone());
    let first = shared
        .open(window(), CapturePolicy::default())
        .await
        .unwrap();
    let slow = shared
        .open(window(), CapturePolicy::default())
        .await
        .unwrap();
    assert_eq!(source.opens.load(Ordering::Relaxed), 1);
    sender.send(frame(1, 0)).await.unwrap();
    first.next(Duration::from_secs(1)).await.unwrap();
    slow.next(Duration::from_secs(1)).await.unwrap();
    sender.send(frame(2, 7)).await.unwrap();
    first.next(Duration::from_secs(1)).await.unwrap();
    sender.send(frame(3, 0)).await.unwrap();
    let latest = first.next(Duration::from_secs(1)).await.unwrap();
    let skipped = slow.next(Duration::from_secs(1)).await.unwrap();
    assert_eq!(latest.frame_id, skipped.frame_id);
    let changes = argusflow_core::capture::changes::changes_since(
        skipped.change_history().unwrap(),
        FrameId::new(1),
        skipped.frame_id,
    )
    .unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(skipped.pixels(), frame(1, 0).pixels());
    drop(first);
    sender.send(frame(4, 9)).await.unwrap();
    assert_eq!(
        slow.next(Duration::from_secs(1)).await.unwrap().frame_id,
        FrameId::new(4)
    );
}
