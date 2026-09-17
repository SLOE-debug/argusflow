//! 录制会话拥有原生帧源，暂停/停止时显式回收，不挂在工作台 OnceCell 中。
use super::{store::Store, worker};
use crate::capture::{clock::Clock, gpu::failure};
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use windows::Win32::Graphics::Dxgi::*;
#[cfg(test)]
#[path = "../../../../../tests/argusflow-windows/unit/capture/frames.rs"]
mod tests;

pub(super) struct Shared {
    pub clock: Clock,
    pub cpu: ByteBudget,
    pub stop: AtomicBool,
    pub store: Mutex<Store>,
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
/// 完整帧后台；克隆共享同一会话，最后一个拥有者释放也会请求停止。
#[derive(Clone)]
pub struct DxgiFrameSource {
    inner: Arc<Inner>,
}
impl DxgiFrameSource {
    /// 启动限帧率的全屏来源；首帧与故障通过 history 读取。
    pub fn start(config: FrameConfig) -> CaptureResult<Self> {
        if !(1..=30).contains(&config.fps)
            || !(2..=16).contains(&config.history)
            || config.width == 0
            || config.width > 3840
            || config.height == 0
            || config.height > 2160
            || config.cpu_bytes < 16 * 1024 * 1024
            || config.cpu_bytes > 512 * 1024 * 1024
            || config.gpu_bytes < 16 * 1024 * 1024
            || config.gpu_bytes > 1024 * 1024 * 1024
        {
            return Err(CaptureError::new(
                FailureKind::InvalidInput,
                "frame_config",
                "全屏帧配置超限",
            ));
        }
        let shared = Arc::new(Shared {
            clock: Clock::new()?,
            cpu: ByteBudget::new(config.cpu_bytes)?,
            stop: AtomicBool::new(false),
            store: Mutex::new(Store::default()),
        });
        let inner = Arc::new(Inner {
            shared: shared.clone(),
            threads: Mutex::new(Vec::new()),
        });
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(failure)?;
        for index in 0..16 {
            let adapter = match unsafe { factory.EnumAdapters1(index) } {
                Ok(a) => a,
                Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(e) => return Err(failure(e)),
            };
            if unsafe { adapter.GetDesc1() }.map_err(failure)?.Flags
                & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32
                != 0
            {
                continue;
            }
            let shared = shared.clone();
            let config = config.clone();
            let thread = std::thread::Builder::new()
                .name(format!("argusflow-frames-{index}"))
                .spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        worker::run(adapter, index, &shared, &config)
                    }));
                    if result.is_err() {
                        shared
                            .store
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .failure = Some(CaptureError::new(
                            FailureKind::Native,
                            "frame_worker",
                            "全屏采集线程异常",
                        ));
                    }
                })
                .map_err(|e| {
                    CaptureError::new(FailureKind::Unavailable, "frame_start", e.to_string())
                })?;
            inner
                .threads
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(thread);
        }
        if inner
            .threads
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_empty()
        {
            return Err(CaptureError::new(
                FailureKind::Unavailable,
                "frame_start",
                "没有硬件图形适配器",
            ));
        }
        Ok(Self { inner })
    }
}
impl DesktopFrameSource for DxgiFrameSource {
    fn clock(&self) -> ClockDomain {
        self.inner.shared.clock.0
    }
    fn now(&self) -> ClockTime {
        self.inner.shared.clock.now()
    }
    fn history(&self) -> CaptureResult<Vec<FrameHistory>> {
        if self.inner.shared.stop.load(Ordering::Acquire) {
            return Err(CaptureError::new(
                FailureKind::Closed,
                "frame_history",
                "全屏采集已停止",
            ));
        }
        let store = self
            .inner
            .shared
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some(e) = &store.failure {
            return Err(e.clone());
        }
        Ok(store.sources.values().cloned().collect())
    }
    fn shutdown(&self, operation: Operation) -> CaptureFuture<()> {
        let inner = self.inner.clone();
        inner.shared.stop.store(true, Ordering::Release);
        Box::pin(async move {
            while !inner
                .threads
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .iter()
                .all(|t| t.is_finished())
            {
                operation.check("frame_shutdown")?;
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            for thread in
                std::mem::take(&mut *inner.threads.lock().unwrap_or_else(|p| p.into_inner()))
            {
                thread.join().map_err(|_| {
                    CaptureError::new(
                        FailureKind::Native,
                        "frame_shutdown",
                        "全屏采集线程异常退出",
                    )
                })?;
            }
            inner
                .shared
                .store
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .sources
                .clear();
            Ok(())
        })
    }
}
