//! 撤销来源同时释放闲置 GPU 租约；失效句柄不能阻塞设备重建预算。
use super::gpu::TileMap;
use argusflow_capture_contracts::{CaptureError, CaptureResult};
use argusflow_core::FailureKind;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};
pub(super) struct MapLease(Mutex<Option<Arc<TileMap>>>);
impl MapLease {
    pub fn get(&self) -> CaptureResult<Arc<TileMap>> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or_else(|| {
                CaptureError::new(FailureKind::StaleHandle, "gpu_lease", "来源 GPU 租约已撤销")
            })
    }
}
pub(super) struct Epoch {
    valid: AtomicBool,
    leases: Mutex<Vec<Weak<MapLease>>>,
}
impl Epoch {
    pub fn new() -> Self {
        Self {
            valid: AtomicBool::new(true),
            leases: Mutex::new(Vec::new()),
        }
    }
    pub fn valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn pin(&self, map: TileMap) -> Arc<MapLease> {
        let lease = Arc::new(MapLease(Mutex::new(Some(Arc::new(map)))));
        let mut leases = self.leases.lock().unwrap_or_else(|p| p.into_inner());
        leases.retain(|lease| lease.strong_count() > 0);
        leases.push(Arc::downgrade(&lease));
        lease
    }
    pub fn revoke(&self) {
        self.valid.store(false, Ordering::Release);
        for lease in self
            .leases
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .drain(..)
            .filter_map(|lease| lease.upgrade())
        {
            lease.0.lock().unwrap_or_else(|p| p.into_inner()).take();
        }
    }
}
