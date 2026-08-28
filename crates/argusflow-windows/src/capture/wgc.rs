//! Windows.Graphics.Capture HWND 捕获和 D3D11 staging readback。

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
    CapturePolicy, CapturedFrame, FrameId, FrameSubscription, QpcTimestamp, TopologyGeneration,
    VisionError, WindowFrameSource,
};
use async_trait::async_trait;
use tokio::sync::Notify;
use windows::{
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::{
            Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem,
            GraphicsCaptureSession,
        },
        DirectX::{Direct3D11::IDirect3DSurface, DirectXPixelFormat},
        SizeInt32,
    },
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct3D11::{
                D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
                D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, ID3D11Resource, ID3D11Texture2D,
            },
            Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
        },
        System::WinRT::{
            Direct3D11::IDirect3DDxgiInterfaceAccess,
            Graphics::Capture::IGraphicsCaptureItemInterop, RoGetActivationFactory,
        },
        UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow},
    },
    core::{HSTRING, IInspectable, Interface},
};

use super::{
    device::{GraphicsDevice, create_graphics_device},
    dpi::window_dpi,
    error::{capture_error, invalid_capture},
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

/// 创建一个绑定 HWND/PID、独立 frame pool 和 topology tracker 的订阅。
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
    let item = create_capture_item(hwnd)?;
    let size = item
        .Size()
        .map_err(|error| capture_error("failed to query capture item size", error))?;
    let (width, height) = valid_size(size)?;
    if let Some(max_dimension) = policy.max_dimension {
        if width.max(height) > max_dimension {
            return Err(invalid_capture(format!(
                "capture item size {width}x{height} exceeds max dimension {max_dimension}"
            )));
        }
    }
    let graphics = Arc::new(create_graphics_device()?);
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &graphics.capture_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        policy.frame_pool_size as i32,
        size,
    )
    .map_err(|error| capture_error("failed to create WGC frame pool", error))?;
    let session = pool
        .CreateCaptureSession(&item)
        .map_err(|error| capture_error("failed to create WGC capture session", error))?;
    session
        .SetIsCursorCaptureEnabled(policy.include_cursor)
        .map_err(|error| capture_error("failed to configure cursor capture", error))?;
    let notify = Arc::new(Notify::new());
    let event_notify = notify.clone();
    let handler =
        TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(move |_, _| {
            event_notify.notify_one();
            Ok(())
        });
    pool.FrameArrived(&handler)
        .map_err(|error| capture_error("failed to subscribe to WGC frames", error))?;
    session
        .StartCapture()
        .map_err(|error| capture_error("failed to start WGC capture", error))?;
    Ok(Arc::new(WindowFrameSubscription {
        window,
        pool,
        session,
        graphics,
        notify,
        topology: Mutex::new(WindowTopologyTracker::new()),
        readback: Mutex::new(()),
        capture_size: Mutex::new(size),
        frame_pool_size: policy.frame_pool_size as i32,
        next_frame_id: AtomicU64::new(0),
    }))
}

/// WGC 帧流订阅；readback 在同一个 immediate context 上串行化。
#[derive(Debug)]
struct WindowFrameSubscription {
    /// 订阅创建时冻结的 HWND/PID。
    window: WindowIdentity,
    /// WGC frame pool。
    pool: Direct3D11CaptureFramePool,
    /// WGC session；保持捕获生命周期。
    session: windows::Graphics::Capture::GraphicsCaptureSession,
    /// D3D11 readback 设备。
    graphics: Arc<GraphicsDevice>,
    /// FrameArrived 的异步唤醒器。
    notify: Arc<Notify>,
    /// 该订阅自己的拓扑追踪器。
    topology: Mutex<WindowTopologyTracker>,
    /// immediate context 不允许并发 Map/Unmap。
    readback: Mutex<()>,
    /// 当前 frame pool 的尺寸；窗口 resize 后在下一帧前重建 pool。
    capture_size: Mutex<SizeInt32>,
    /// 用于 resize 后重建 frame pool 的 buffer 数量。
    frame_pool_size: i32,
    /// 当前订阅内单调分配的帧 ID。
    next_frame_id: AtomicU64,
}

#[async_trait]
impl FrameSubscription for WindowFrameSubscription {
    async fn next(&self, timeout: Duration) -> Result<Arc<CapturedFrame>, VisionError> {
        let deadline = Instant::now() + timeout;
        loop {
            let frame = match self.pool.TryGetNextFrame() {
                Ok(frame) => frame,
                Err(_) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(frame_timeout(timeout));
                    }
                    tokio::time::timeout(remaining, self.notify.notified())
                        .await
                        .map_err(|_| frame_timeout(timeout))?;
                    continue;
                }
            };
            let frame_id = FrameId::new(self.next_frame_id.fetch_add(1, Ordering::Relaxed) + 1);
            let topology_generation = match self.current_topology_generation() {
                Ok(generation) => generation,
                Err(error) => {
                    let _ = frame.Close();
                    return Err(error);
                }
            };
            let result = {
                let _readback = self
                    .readback
                    .lock()
                    .map_err(|_| invalid_capture("D3D11 readback mutex was poisoned"))?;
                readback_frame(
                    &frame,
                    &self.graphics,
                    self.window,
                    frame_id,
                    topology_generation,
                )
            };
            // SAFETY: frame 由 TryGetNextFrame 返回并且在此处不再被 readback 使用。
            let close_result = frame.Close();
            match result {
                Err(error) => return Err(error),
                Ok(captured) => {
                    close_result
                        .map_err(|error| capture_error("failed to close WGC frame", error))?;
                    self.recreate_pool_if_resized(&captured)?;
                    return Ok(captured);
                }
            }
        }
    }

    fn window(&self) -> WindowIdentity {
        self.window
    }
}

impl WindowFrameSubscription {
    /// 读取当前拓扑代数；每张帧重新观察，避免旧 scene 跨 popup 变化复用。
    fn current_topology_generation(&self) -> Result<TopologyGeneration, VisionError> {
        let mut topology = self
            .topology
            .lock()
            .map_err(|_| invalid_capture("window topology mutex was poisoned"))?;
        Ok(topology.refresh(self.window)?.generation)
    }

    /// 处理 WGC 在窗口 resize 后继续返回新尺寸的问题，避免把旧 pool 当成永久尺寸。
    fn recreate_pool_if_resized(&self, frame: &CapturedFrame) -> Result<(), VisionError> {
        let mut size = self
            .capture_size
            .lock()
            .map_err(|_| invalid_capture("capture size mutex was poisoned"))?;
        let width = i32::try_from(frame.width)
            .map_err(|_| invalid_capture("captured frame width does not fit WinRT size"))?;
        let height = i32::try_from(frame.height)
            .map_err(|_| invalid_capture("captured frame height does not fit WinRT size"))?;
        if size.Width == width && size.Height == height {
            return Ok(());
        }
        let new_size = SizeInt32 {
            Width: width,
            Height: height,
        };
        self.pool
            .Recreate(
                &self.graphics.capture_device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                self.frame_pool_size,
                new_size,
            )
            .map_err(|error| capture_error("failed to recreate resized WGC frame pool", error))?;
        *size = new_size;
        Ok(())
    }
}

impl Drop for WindowFrameSubscription {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

/// 从 WinRT 激活工厂取得 HWND 专用 GraphicsCaptureItem。
fn create_capture_item(hwnd: HWND) -> Result<GraphicsCaptureItem, VisionError> {
    let class_name = HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureItem");
    let interop: IGraphicsCaptureItemInterop = unsafe { RoGetActivationFactory(&class_name) }
        .map_err(|error| capture_error("failed to get GraphicsCaptureItem factory", error))?;
    unsafe { interop.CreateForWindow(hwnd) }
        .map_err(|error| capture_error("failed to create GraphicsCaptureItem for HWND", error))
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

/// 校验 WGC 传入的尺寸能安全转换为领域层的无符号尺寸。
fn valid_size(size: SizeInt32) -> Result<(u32, u32), VisionError> {
    let width =
        u32::try_from(size.Width).map_err(|_| invalid_capture("capture width is invalid"))?;
    let height =
        u32::try_from(size.Height).map_err(|_| invalid_capture("capture height is invalid"))?;
    (width > 0 && height > 0)
        .then_some((width, height))
        .ok_or_else(|| invalid_capture("capture item has an empty size"))
}

/// 把 WGC GPU surface 复制到 staging texture，并复制成短期拥有型 BGRA8 buffer。
fn readback_frame(
    frame: &Direct3D11CaptureFrame,
    graphics: &GraphicsDevice,
    window: WindowIdentity,
    frame_id: FrameId,
    topology_generation: TopologyGeneration,
) -> Result<Arc<CapturedFrame>, VisionError> {
    let surface: IDirect3DSurface = frame
        .Surface()
        .map_err(|error| capture_error("failed to get WGC frame surface", error))?;
    let access: IDirect3DDxgiInterfaceAccess = surface
        .cast()
        .map_err(|error| capture_error("failed to access WGC DXGI surface", error))?;
    let source_texture: ID3D11Texture2D = unsafe { access.GetInterface() }
        .map_err(|error| capture_error("failed to query WGC D3D11 texture", error))?;
    let mut source_desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: source_desc 是 texture API 的独占输出缓冲区。
    unsafe { source_texture.GetDesc(&mut source_desc) };
    if source_desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM || source_desc.SampleDesc.Count != 1 {
        return Err(invalid_capture(
            "WGC returned a non-BGRA8 or multisampled texture",
        ));
    }
    let timestamp_qpc = frame
        .SystemRelativeTime()
        .map(|time| QpcTimestamp::new(time.Duration.max(0) as u64))
        .map_err(|error| capture_error("failed to read WGC frame timestamp", error))?;
    let row_bytes = usize::try_from(source_desc.Width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or_else(|| invalid_capture("capture row byte length overflow"))?;
    let pixel_bytes = row_bytes
        .checked_mul(source_desc.Height as usize)
        .ok_or_else(|| invalid_capture("capture pixel byte length overflow"))?;
    let staging_desc = D3D11_TEXTURE2D_DESC {
        Width: source_desc.Width,
        Height: source_desc.Height,
        MipLevels: 1,
        ArraySize: 1,
        Format: source_desc.Format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut staging: Option<ID3D11Texture2D> = None;
    // SAFETY: staging_desc 和输出槽在调用期间保持有效；创建的 texture 由 Option 接管所有权。
    unsafe {
        graphics.device.CreateTexture2D(
            &staging_desc,
            None,
            Some(&mut staging as *mut Option<ID3D11Texture2D>),
        )
    }
    .map_err(|error| capture_error("failed to create D3D11 staging texture", error))?;
    let staging = staging.ok_or_else(|| invalid_capture("D3D11 returned no staging texture"))?;
    let source_resource: ID3D11Resource = source_texture
        .cast()
        .map_err(|error| capture_error("failed to cast source texture resource", error))?;
    let staging_resource: ID3D11Resource = staging
        .cast()
        .map_err(|error| capture_error("failed to cast staging texture resource", error))?;
    // SAFETY: 两个资源均为当前设备创建/拥有的 D3D11 资源，CopyResource 不跨设备。
    unsafe {
        graphics
            .context
            .CopyResource(&staging_resource, &source_resource)
    };
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    // SAFETY: staging texture 使用 CPU_READ，且 immediate context 由 readback mutex 独占。
    unsafe {
        graphics
            .context
            .Map(&staging_resource, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
    }
    .map_err(|error| capture_error("failed to map D3D11 staging texture", error))?;
    let result = if mapped.pData.is_null() || (mapped.RowPitch as usize) < row_bytes {
        Err(invalid_capture("D3D11 map returned an invalid row pitch"))
    } else {
        match (mapped.RowPitch as usize).checked_mul(source_desc.Height as usize) {
            None => Err(invalid_capture("mapped row byte length overflow")),
            Some(mapped_len) => {
                // SAFETY: Map 成功后 pData 指向至少 mapped_len 字节的 texture 子资源内存。
                let mapped_pixels =
                    unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>(), mapped_len) };
                let mut pixels = vec![0_u8; pixel_bytes];
                for row in 0..source_desc.Height as usize {
                    let source_start = row * mapped.RowPitch as usize;
                    let target_start = row * row_bytes;
                    pixels[target_start..target_start + row_bytes]
                        .copy_from_slice(&mapped_pixels[source_start..source_start + row_bytes]);
                }
                let dpi = window_dpi(native_window(window.handle));
                CapturedFrame::from_bgra8(
                    frame_id,
                    topology_generation,
                    window,
                    timestamp_qpc,
                    source_desc.Width,
                    source_desc.Height,
                    dpi,
                    dpi,
                    row_bytes,
                    pixels,
                )
                .map(Arc::new)
            }
        }
    };
    // SAFETY: Map 已成功返回，且 staging_resource 仍由当前函数持有。
    unsafe { graphics.context.Unmap(&staging_resource, 0) };
    result
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
