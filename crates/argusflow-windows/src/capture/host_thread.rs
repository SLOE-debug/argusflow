//! 捕获主机线程内部状态与串行命令执行。

use std::{
    collections::HashMap,
    sync::{
        Arc,
        mpsc::{Receiver, SyncSender},
    },
    time::{Duration, Instant},
};

use argusflow_core::WindowIdentity;
use argusflow_vision::{CapturePolicy, CapturedFrame, FrameId, TopologyGeneration, VisionError};
use tokio::sync::Notify;
use windows::{
    Graphics::Capture::GraphicsCaptureSession,
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
};

use super::{
    device::{GraphicsDevice, create_graphics_device},
    error::{capture_error, invalid_capture},
    host::{CaptureCommand, CaptureSubscriptionId, OpenedCapture, capture_host_error},
    readback::ReadbackState,
    window_identity::{native_window, validate_window},
    window_surface::WindowCaptureSurface,
};

/// 仅存在于捕获线程中的可变资源集合。
struct CaptureThreadState {
    /// 应用生命周期内复用的唯一 D3D11/WinRT 图形设备。
    graphics: Arc<GraphicsDevice>,
    /// 当前存活的窗口订阅；值中的所有 WinRT 对象都不跨线程移动。
    subscriptions: HashMap<CaptureSubscriptionId, HostedSubscription>,
    /// 单调分配的主机级订阅 ID。
    next_subscription_id: u64,
}

impl CaptureThreadState {
    /// 在已经初始化的 MTA 中创建共享图形设备。
    fn new() -> Result<Self, VisionError> {
        let supported = GraphicsCaptureSession::IsSupported()
            .map_err(|error| capture_error("failed to query WGC support", error))?;
        if !supported {
            return Err(invalid_capture("Windows.Graphics.Capture is not supported"));
        }
        Ok(Self {
            graphics: Arc::new(create_graphics_device()?),
            subscriptions: HashMap::new(),
            next_subscription_id: 0,
        })
    }

    /// 创建由主机线程完整拥有的捕获订阅。
    fn open(
        &mut self,
        window: WindowIdentity,
        policy: CapturePolicy,
    ) -> Result<OpenedCapture, VisionError> {
        if !matches!(policy.frame_pool_size, 2..=16) {
            return Err(invalid_capture("frame_pool_size must be between 2 and 16"));
        }
        validate_window(native_window(window.handle), window)?;
        let notify = Arc::new(Notify::new());
        let surface = WindowCaptureSurface::new(
            window,
            &self.graphics,
            notify.clone(),
            policy.frame_pool_size as i32,
            policy.include_cursor,
            policy.max_dimension,
        )?;
        self.next_subscription_id = self
            .next_subscription_id
            .checked_add(1)
            .ok_or_else(|| capture_host_error("capture subscription id space was exhausted"))?;
        let subscription_id = CaptureSubscriptionId(self.next_subscription_id);
        self.subscriptions.insert(
            subscription_id,
            HostedSubscription {
                surface,
                graphics: self.graphics.clone(),
                readback: ReadbackState::default(),
                generation: TopologyGeneration::new(1),
                last_bounds: None,
            },
        );
        Ok(OpenedCapture {
            subscription_id,
            notify,
        })
    }

    /// 读取单 HWND 的下一张帧，并维护移动/缩放 generation。
    fn poll(
        &mut self,
        subscription_id: CaptureSubscriptionId,
        frame_id: FrameId,
        deadline: Instant,
        timeout: Duration,
    ) -> Result<Option<Arc<CapturedFrame>>, VisionError> {
        if Instant::now() >= deadline {
            return Err(frame_timeout(timeout));
        }
        let subscription = self.subscription_mut(subscription_id)?;
        let bounds = subscription.surface.bounds()?;
        if subscription
            .last_bounds
            .is_some_and(|previous| previous != bounds)
        {
            subscription.generation =
                TopologyGeneration::new(subscription.generation.get().saturating_add(1));
            subscription.readback.clear();
        }
        subscription.last_bounds = Some(bounds);
        subscription.surface.poll(
            &subscription.graphics,
            &mut subscription.readback,
            frame_id,
            subscription.generation,
            deadline,
            timeout,
        )
    }

    /// 返回单窗口当前 generation。
    fn current_topology_generation(
        &mut self,
        subscription_id: CaptureSubscriptionId,
    ) -> Result<TopologyGeneration, VisionError> {
        let subscription = self.subscription_mut(subscription_id)?;
        let bounds = subscription.surface.bounds()?;
        if subscription
            .last_bounds
            .is_some_and(|previous| previous != bounds)
        {
            subscription.generation =
                TopologyGeneration::new(subscription.generation.get().saturating_add(1));
        }
        subscription.last_bounds = Some(bounds);
        Ok(subscription.generation)
    }

    /// 取得仍存活的主机订阅。
    fn subscription_mut(
        &mut self,
        subscription_id: CaptureSubscriptionId,
    ) -> Result<&mut HostedSubscription, VisionError> {
        self.subscriptions
            .get_mut(&subscription_id)
            .ok_or_else(|| capture_host_error("capture subscription is no longer available"))
    }
}

/// 主机线程拥有的单个窗口捕获状态。
struct HostedSubscription {
    /// 当前 HWND 唯一的 WGC capture surface。
    surface: WindowCaptureSurface,
    /// 复用共享 D3D11 device 的引用。
    graphics: Arc<GraphicsDevice>,
    /// 该订阅复用的 staging texture。
    readback: ReadbackState,
    /// 窗口移动或 resize 时递增。
    generation: TopologyGeneration,
    /// 上次读取到的 DWM 可见物理边界。
    last_bounds: Option<argusflow_vision::PhysicalRect>,
}

/// 初始化主机线程并串行处理其完整生命周期命令。
pub(super) fn run_capture_thread(
    commands: Receiver<CaptureCommand>,
    ready: SyncSender<Result<(), VisionError>>,
) {
    let apartment = match WinRtApartment::initialize() {
        Ok(apartment) => apartment,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let mut state = match CaptureThreadState::new() {
        Ok(state) => state,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }
    while let Ok(command) = commands.recv() {
        match command {
            CaptureCommand::Open {
                window,
                policy,
                reply,
            } => match state.open(window, policy) {
                Ok(opened) => {
                    let subscription_id = opened.subscription_id;
                    if reply.send(Ok(opened)).is_err() {
                        state.subscriptions.remove(&subscription_id);
                    }
                }
                Err(error) => {
                    let _ = reply.send(Err(error));
                }
            },
            CaptureCommand::Poll {
                subscription_id,
                frame_id,
                deadline,
                timeout,
                reply,
            } => {
                let _ = reply.send(state.poll(subscription_id, frame_id, deadline, timeout));
            }
            CaptureCommand::CurrentTopology {
                subscription_id,
                reply,
            } => {
                let _ = reply.send(state.current_topology_generation(subscription_id));
            }
            CaptureCommand::Close { subscription_id } => {
                state.subscriptions.remove(&subscription_id);
            }
            CaptureCommand::Shutdown { reply } => {
                state.subscriptions.clear();
                drop(state);
                let _ = reply.send(());
                drop(apartment);
                return;
            }
        }
    }
}

/// 当前线程一次成功的 WinRT MTA 初始化；Drop 与 RoInitialize 严格配对。
struct WinRtApartment;

impl WinRtApartment {
    /// 把捕获线程初始化为多线程 apartment。
    fn initialize() -> Result<Self, VisionError> {
        // SAFETY: 每个主机线程只调用一次，并由本类型的 Drop 在同一线程配对释放。
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|error| {
            capture_error("failed to initialize capture thread WinRT MTA", error)
        })?;
        Ok(Self)
    }
}

impl Drop for WinRtApartment {
    fn drop(&mut self) {
        // SAFETY: 本 guard 只在成功 RoInitialize 的同一主机线程内创建和销毁。
        unsafe { RoUninitialize() };
    }
}

/// 把等待时限转换为视觉层统一的超时错误。
fn frame_timeout(timeout: Duration) -> VisionError {
    VisionError::FrameTimeout {
        timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
    }
}
