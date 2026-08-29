//! Windows.Graphics.Capture surface set 的订阅编排。

use std::{
    ffi::c_void,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use argusflow_core::WindowIdentity;
use argusflow_vision::{
    CapturePolicy, CapturedFrame, FrameId, FrameSubscription, TopologyGeneration, VisionError,
    WindowFrameSource,
};
use async_trait::async_trait;
use tokio::sync::Notify;
use windows::{
    Graphics::Capture::GraphicsCaptureSession,
    Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow},
    },
};

use super::{
    device::create_graphics_device,
    error::{capture_error, invalid_capture},
    readback::ReadbackState,
    surface_set::CaptureSurfaceSet,
    topology::WindowTopologyTracker,
};

/// Windows.Graphics.Capture 的窗口级帧源。
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsGraphicsCapture;

impl WindowsGraphicsCapture {
    /// 创建无状态 WGC 帧源；设备和窗口订阅在 open 阶段按作用域建立。
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl WindowFrameSource for WindowsGraphicsCapture {
    async fn open(
        &self,
        window: WindowIdentity,
        policy: CapturePolicy,
    ) -> Result<Arc<dyn FrameSubscription>, VisionError> {
        open_capture(window, policy)
    }
}

/// 创建一个绑定 primary HWND/PID、独立 frame pool 和 topology tracker 的订阅。
fn open_capture(
    window: WindowIdentity,
    policy: CapturePolicy,
) -> Result<Arc<dyn FrameSubscription>, VisionError> {
    if !matches!(policy.frame_pool_size, 2..=16) {
        return Err(invalid_capture("frame_pool_size must be between 2 and 16"));
    }
    let supported = GraphicsCaptureSession::IsSupported()
        .map_err(|error| capture_error("failed to query WGC support", error))?;
    if !supported {
        return Err(invalid_capture("Windows.Graphics.Capture is not supported"));
    }
    let hwnd = native_window(window.handle);
    validate_window(hwnd, window)?;
    let graphics = Arc::new(create_graphics_device()?);
    let notify = Arc::new(Notify::new());
    let mut topology = WindowTopologyTracker::new();
    let initial_topology = topology.refresh(window)?;
    let surfaces = CaptureSurfaceSet::new(
        &initial_topology,
        graphics.clone(),
        notify.clone(),
        policy.frame_pool_size as i32,
        policy.include_cursor,
        policy.max_dimension,
    )?;
    Ok(Arc::new(WindowFrameSubscription {
        window,
        notify,
        topology: Mutex::new(topology),
        surfaces: Mutex::new(surfaces),
        readback: Mutex::new(ReadbackState::default()),
        next_frame_id: AtomicU64::new(0),
    }))
}

/// WGC 帧流订阅；readback 在同一个 immediate context 上串行化。
#[derive(Debug)]
struct WindowFrameSubscription {
    /// 订阅创建时冻结的 HWND/PID。
    window: WindowIdentity,
    /// FrameArrived 的异步唤醒器。
    notify: Arc<Notify>,
    /// 该订阅自己的拓扑追踪器。
    topology: Mutex<WindowTopologyTracker>,
    /// primary 与 owned popup 的独立 WGC surface 集合。
    surfaces: Mutex<CaptureSurfaceSet>,
    /// immediate context 与该订阅专属 staging texture 不允许并发使用。
    readback: Mutex<ReadbackState>,
    /// 当前订阅内单调分配的帧 ID。
    next_frame_id: AtomicU64,
}

#[async_trait]
impl FrameSubscription for WindowFrameSubscription {
    async fn next(&self, timeout: Duration) -> Result<Arc<CapturedFrame>, VisionError> {
        let deadline = Instant::now() + timeout;
        loop {
            let topology_generation = self.current_topology_generation()?;
            let frame_id = FrameId::new(self.next_frame_id.fetch_add(1, Ordering::Relaxed) + 1);
            let captured = {
                let mut surfaces = self
                    .surfaces
                    .lock()
                    .map_err(|_| invalid_capture("capture surface mutex was poisoned"))?;
                let mut readback = self
                    .readback
                    .lock()
                    .map_err(|_| invalid_capture("D3D11 readback mutex was poisoned"))?;
                let updated = surfaces.poll_frames(
                    &mut readback,
                    self.window,
                    frame_id,
                    topology_generation,
                )?;
                if updated && surfaces.has_complete_frame() {
                    Some(surfaces.compose(frame_id, topology_generation, self.window)?)
                } else {
                    None
                }
            };
            if let Some(captured) = captured {
                return Ok(captured);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(frame_timeout(timeout));
            }
            tokio::time::timeout(remaining, self.notify.notified())
                .await
                .map_err(|_| frame_timeout(timeout))?;
        }
    }

    fn window(&self) -> WindowIdentity {
        self.window
    }

    async fn current_topology_generation(&self) -> Result<TopologyGeneration, VisionError> {
        WindowFrameSubscription::current_topology_generation(self)
    }
}

impl WindowFrameSubscription {
    /// 读取当前拓扑代数；每张帧重新观察，避免旧 scene 跨 popup 变化复用。
    fn current_topology_generation(&self) -> Result<TopologyGeneration, VisionError> {
        let mut topology = self
            .topology
            .lock()
            .map_err(|_| invalid_capture("window topology mutex was poisoned"))?;
        let current = topology.refresh(self.window)?;
        let generation = current.generation;
        self.surfaces
            .lock()
            .map_err(|_| invalid_capture("capture surface mutex was poisoned"))?
            .reconcile(&current)?;
        Ok(generation)
    }
}

/// 只接受仍指向原 PID 的 HWND，防止句柄复用导致跨应用捕获。
fn validate_window(hwnd: HWND, expected: WindowIdentity) -> Result<(), VisionError> {
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return Err(VisionError::WindowIdentityChanged {
            expected,
            actual: None,
        });
    }
    let mut process_id = 0_u32;
    // SAFETY: process_id 是同步 Win32 调用的独占输出，hwnd 已通过 IsWindow 校验。
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    if process_id != expected.process_id {
        return Err(VisionError::WindowIdentityChanged {
            expected,
            actual: Some(WindowIdentity {
                handle: expected.handle,
                process_id,
            }),
        });
    }
    Ok(())
}

/// 把领域层的不透明 HWND 表示恢复为 Windows 类型。
fn native_window(handle: u64) -> HWND {
    HWND(handle as usize as *mut c_void)
}

/// 把等待时限转换为视觉层统一的超时错误。
fn frame_timeout(timeout: Duration) -> VisionError {
    VisionError::FrameTimeout {
        timeout_ms: timeout.as_millis() as u64,
    }
}
