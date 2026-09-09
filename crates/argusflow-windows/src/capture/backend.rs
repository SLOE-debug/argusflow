//! 共享原生主机，线程持有实例所有权直到真实资源回收。
use super::{
    clock::Clock,
    gpu::{failure, invalid},
    queue::EventQueue,
    worker,
};
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIFactory1,
};

static OWNED: AtomicBool = AtomicBool::new(false);
struct Owner;
impl Drop for Owner {
    fn drop(&mut self) {
        OWNED.store(false, Ordering::Release);
    }
}
pub(super) struct Shared {
    pub clock: Clock,
    pub queue: Arc<EventQueue>,
    pub cpu: ByteBudget,
    pub stop: AtomicBool,
    pub restart: AtomicU64,
    pub stats: Mutex<Vec<CaptureStats>>,
    pub fatal: Mutex<Option<CaptureError>>,
    consumer: AtomicBool,
    consumed: AtomicBool,
}
struct Inner {
    shared: Arc<Shared>,
    threads: Mutex<Vec<std::thread::JoinHandle<()>>>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
    }
}
/// 单进程一个 DXGI 后台，克隆共享资源；必须显式 shutdown。
#[derive(Clone)]
pub struct DxgiBackend {
    inner: Arc<Inner>,
}
impl DxgiBackend {
    /// 启动每适配器工作线程；来源初始化结果通过 Source 事件报告。
    pub fn start(config: BackendConfig) -> CaptureResult<Self> {
        validate(&config)?;
        if OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(CaptureError::new(
                FailureKind::Busy,
                "dxgi_start",
                "DXGI 后台已经存在",
            ));
        }
        let owner = Arc::new(Owner);
        let clock = Clock::new()?;
        let shared = Arc::new(Shared {
            clock,
            queue: Arc::new(EventQueue::new(config.event_bytes)),
            cpu: ByteBudget::new(config.cpu_bytes)?,
            stop: AtomicBool::new(false),
            restart: AtomicU64::new(0),
            stats: Mutex::new(Vec::new()),
            fatal: Mutex::new(None),
            consumer: AtomicBool::new(false),
            consumed: AtomicBool::new(false),
        });
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(failure)?;
        let mut adapters = Vec::new();
        let mut index = 0;
        loop {
            let adapter = match unsafe { factory.EnumAdapters1(index) } {
                Ok(adapter) => adapter,
                Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(error) => return Err(failure(error)),
            };
            let desc = unsafe { adapter.GetDesc1() }.map_err(failure)?;
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 == 0 {
                adapters.push((index, adapter));
            }
            index += 1;
            if index > 16 {
                return Err(invalid("too many graphics adapters"));
            }
        }
        if adapters.is_empty() {
            return Err(CaptureError::new(
                FailureKind::Unavailable,
                "dxgi_start",
                "没有硬件图形适配器",
            ));
        }
        shared
            .stats
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .resize(adapters.len(), CaptureStats::default());
        let inner = Arc::new(Inner {
            shared: shared.clone(),
            threads: Mutex::new(Vec::new()),
        });
        for (slot, (index, adapter)) in adapters.into_iter().enumerate() {
            let shared = shared.clone();
            let owner = owner.clone();
            let config = config.clone();
            let handle = std::thread::Builder::new()
                .name(format!("argusflow-dxgi-{index}"))
                .spawn(move || {
                    let _owner = owner;
                    worker::run(adapter, index, slot, shared, config);
                })
                .map_err(|error| {
                    CaptureError::Failure(
                        argusflow_core::Failure::new(
                            FailureKind::Unavailable,
                            "dxgi_start",
                            "无法创建采样线程",
                        )
                        .with_source(error),
                    )
                })?;
            inner
                .threads
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(handle);
        }
        Ok(Self { inner })
    }
}
impl DesktopBackend for DxgiBackend {
    fn claim_consumer(&self) -> CaptureResult<()> {
        self.inner
            .shared
            .consumer
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                CaptureError::new(
                    FailureKind::Busy,
                    "capture_consumer",
                    "原生事件消费端已被占用",
                )
            })?;
        if self.inner.shared.consumed.swap(true, Ordering::AcqRel) {
            self.inner.shared.queue.resynchronize(self.now());
        }
        Ok(())
    }
    fn release_consumer(&self) {
        self.inner.shared.consumer.store(false, Ordering::Release);
    }
    fn clock(&self) -> ClockDomain {
        self.inner.shared.clock.0
    }
    fn now(&self) -> ClockTime {
        self.inner.shared.clock.now()
    }
    fn poll(&self) -> CaptureResult<Vec<BackendEvent>> {
        if let Some(error) = self.inner.shared.queue.failure() {
            return Err(error);
        }
        if let Some(error) = &*self
            .inner
            .shared
            .fatal
            .lock()
            .unwrap_or_else(|p| p.into_inner())
        {
            return Err(error.clone());
        }
        Ok(self.inner.shared.queue.drain())
    }
    fn stats(&self) -> CaptureStats {
        let states = self
            .inner
            .shared
            .stats
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let mut result = CaptureStats::default();
        for state in states.iter() {
            result.acquired_frames += state.acquired_frames;
            result.changed_frames += state.changed_frames;
            result.metadata_readback_bytes += state.metadata_readback_bytes;
            result.pixel_readback_bytes += state.pixel_readback_bytes;
            result.gaps += state.gaps;
            result.gpu_bytes += state.gpu_bytes;
            result.gpu_peak_bytes += state.gpu_peak_bytes;
        }
        result.cpu_bytes = self.inner.shared.cpu.used();
        result.gaps = self.inner.shared.queue.gap_count();
        result.cpu_peak_bytes = self.inner.shared.cpu.peak();
        result
    }
    fn restart(&self) -> CaptureResult<()> {
        if self.inner.shared.stop.load(Ordering::Acquire) {
            return Err(CaptureError::new(
                FailureKind::Closed,
                "dxgi_restart",
                "采样已关闭",
            ));
        }
        self.inner.shared.restart.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn shutdown(&self, operation: Operation) -> CaptureFuture<()> {
        let inner = self.inner.clone();
        inner.shared.stop.store(true, Ordering::Release);
        Box::pin(async move {
            loop {
                if inner
                    .threads
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .iter()
                    .all(|thread| thread.is_finished())
                {
                    break;
                }
                if operation.remaining().is_zero() || operation.is_cancelled() {
                    return Err(CaptureError::new(
                        FailureKind::Unresponsive,
                        "dxgi_shutdown",
                        "原生线程尚未回收",
                    ));
                }
                tokio::time::sleep(std::time::Duration::from_millis(8)).await;
            }
            let threads =
                std::mem::take(&mut *inner.threads.lock().unwrap_or_else(|p| p.into_inner()));
            for thread in threads {
                thread.join().map_err(|_| {
                    CaptureError::new(FailureKind::Native, "dxgi_shutdown", "采样线程 panic")
                })?;
            }
            Ok(())
        })
    }
}
fn validate(config: &BackendConfig) -> CaptureResult<()> {
    if config.gpu_bytes < 4 * 1024 * 1024
        || config.gpu_bytes > 2 * 1024 * 1024 * 1024
        || config.cpu_bytes == 0
        || config.cpu_bytes > 1024 * 1024 * 1024
        || !(1..=3).contains(&config.max_in_flight)
        || config.acquire_wait.is_zero()
        || config.acquire_wait > std::time::Duration::from_millis(8)
        || config.gpu_timeout.is_zero()
        || config.gpu_timeout > std::time::Duration::from_secs(5)
        || config.recovery_timeout.is_zero()
        || config.recovery_timeout > std::time::Duration::from_secs(30)
        || config.event_bytes < 4096
        || config.event_bytes > 16 * 1024 * 1024
        || config.read_queue == 0
        || config.read_queue > 256
    {
        return Err(CaptureError::new(
            FailureKind::InvalidInput,
            "dxgi_config",
            "原生采样配置超限",
        ));
    }
    Ok(())
}
