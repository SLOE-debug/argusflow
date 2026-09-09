//! 引用计数资源预留，交付给消费者后仍占预算。
use crate::{CaptureError, CaptureResult};
use argusflow_core::FailureKind;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct State {
    limit: usize,
    used: AtomicUsize,
    peak: AtomicUsize,
}
/// 可克隆的统一字节额度。
#[derive(Clone)]
pub struct ByteBudget(Arc<State>);
impl ByteBudget {
    /// 创建非零预算。
    pub fn new(limit: usize) -> CaptureResult<Self> {
        if limit == 0 {
            return Err(CaptureError::new(
                FailureKind::InvalidInput,
                "budget",
                "预算必须非零",
            ));
        }
        Ok(Self(Arc::new(State {
            limit,
            used: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        })))
    }
    /// 先预留后分配，额度不足不等待。
    pub fn reserve(&self, bytes: usize) -> CaptureResult<Reservation> {
        let previous = self
            .0
            .used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes).filter(|next| *next <= self.0.limit)
            })
            .map_err(|_| {
                CaptureError::new(FailureKind::ResourceLimit, "budget", "采样资源预算已满")
            })?;
        self.0.peak.fetch_max(previous + bytes, Ordering::Relaxed);
        Ok(Reservation {
            state: self.0.clone(),
            bytes,
        })
    }
    /// 当前存活字节数。
    pub fn used(&self) -> usize {
        self.0.used.load(Ordering::Acquire)
    }
    /// 自创建以来的峰值。
    pub fn peak(&self) -> usize {
        self.0.peak.load(Ordering::Relaxed)
    }
    /// 配置上限。
    pub fn limit(&self) -> usize {
        self.0.limit
    }
}
/// 最后一个资源所有者释放时归还额度；不能独立复制预留。
pub struct Reservation {
    state: Arc<State>,
    bytes: usize,
}
impl Reservation {
    /// 本次预留字节数。
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.state.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}
