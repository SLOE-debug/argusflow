//! WGC primary/popup surface set、逐 surface readback 与合成。

use std::sync::Arc;

use argusflow_core::{ScreenPoint, WindowIdentity};
use argusflow_vision::{CapturedFrame, FrameId, PhysicalRect, TopologyGeneration, VisionError};
use tokio::sync::Notify;
use windows::{
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::DirectXPixelFormat,
        SizeInt32,
    },
    Win32::{
        Foundation::HWND,
        System::WinRT::{Graphics::Capture::IGraphicsCaptureItemInterop, RoGetActivationFactory},
        UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow},
    },
    core::{HSTRING, IInspectable},
};

use super::{
    device::GraphicsDevice,
    error::{capture_error, invalid_capture},
    readback::{ReadbackState, readback_frame},
    topology::{WindowRole, WindowTopology, WindowTopologyEntry},
};

/// primary 和 owned popup 共同组成的实际 WGC surface set。
#[derive(Debug)]
pub(super) struct CaptureSurfaceSet {
    /// 所有已成功建立 WGC session 的 surface，primary 始终排在最前面。
    surfaces: Vec<CaptureSurface>,
    /// 所有 surface 共用的 D3D11 设备。
    graphics: Arc<GraphicsDevice>,
    /// 任一 surface 到帧时唤醒订阅者。
    notify: Arc<Notify>,
    /// 新 surface 使用的 frame pool 缓冲数。
    frame_pool_size: i32,
    /// 是否在新 surface 中捕获光标。
    include_cursor: bool,
    /// 单个 surface 允许的最大边长。
    max_dimension: Option<u32>,
    /// 最近一次完成 reconciliation 的拓扑代数。
    generation: Option<TopologyGeneration>,
}

/// 一个拥有自己 frame pool/session 的 WGC surface。
#[derive(Debug)]
struct CaptureSurface {
    /// surface 对应的窗口身份、角色和屏幕矩形。
    entry: WindowTopologyEntry,
    /// 该 surface 的 WGC frame pool。
    pool: Direct3D11CaptureFramePool,
    /// 保持捕获生命周期的 WGC session。
    session: GraphicsCaptureSession,
    /// 当前 frame pool 的尺寸。
    size: SizeInt32,
    /// 用于 resize 后重建 frame pool 的 buffer 数量。
    frame_pool_size: i32,
    /// 最近一次成功 readback 的 surface 图像。
    latest: Option<Arc<CapturedFrame>>,
}

impl CaptureSurfaceSet {
    /// 根据一次拓扑快照建立 primary 和 owned popup capture surfaces。
    pub(super) fn new(
        topology: &WindowTopology,
        graphics: Arc<GraphicsDevice>,
        notify: Arc<Notify>,
        frame_pool_size: i32,
        include_cursor: bool,
        max_dimension: Option<u32>,
    ) -> Result<Self, VisionError> {
        let mut set = Self {
            surfaces: Vec::new(),
            graphics,
            notify,
            frame_pool_size,
            include_cursor,
            max_dimension,
            generation: None,
        };
        set.reconcile(topology)?;
        Ok(set)
    }

    /// 拓扑变化时增删 popup session，并清除跨代缓存帧。
    pub(super) fn reconcile(&mut self, topology: &WindowTopology) -> Result<(), VisionError> {
        let desired = capture_entries(topology)?;
        let generation_changed = self.generation != Some(topology.generation);
        let mut existing = std::mem::take(&mut self.surfaces);
        let mut surfaces = Vec::with_capacity(desired.len());
        for entry in desired {
            if let Some(index) = existing.iter().position(|surface| {
                surface.entry.identity == entry.identity && surface.entry.role == entry.role
            }) {
                let mut surface = existing.swap_remove(index);
                surface.entry = entry;
                if generation_changed {
                    surface.latest = None;
                }
                surfaces.push(surface);
            } else {
                surfaces.push(CaptureSurface::new(
                    entry,
                    self.graphics.clone(),
                    self.notify.clone(),
                    self.frame_pool_size,
                    self.include_cursor,
                    self.max_dimension,
                )?);
            }
        }
        self.surfaces = surfaces;
        self.generation = Some(topology.generation);
        Ok(())
    }

    /// 轮询每个 surface 的新帧；只要至少一个 surface 有新 readback 就返回 true。
    pub(super) fn poll_frames(
        &mut self,
        readback: &mut ReadbackState,
        window: WindowIdentity,
        frame_id: FrameId,
        topology_generation: TopologyGeneration,
    ) -> Result<bool, VisionError> {
        let mut updated = false;
        for surface in &mut self.surfaces {
            let frame = match surface.pool.TryGetNextFrame() {
                Ok(frame) => frame,
                Err(_) => continue,
            };
            let captured = readback_frame(
                &frame,
                &self.graphics,
                readback,
                window,
                frame_id,
                topology_generation,
            );
            let close_result = frame.Close();
            let captured = captured?;
            close_result.map_err(|error| capture_error("failed to close WGC frame", error))?;
            surface.recreate_if_resized(&captured, readback, &self.graphics)?;
            let origin = surface
                .entry
                .bounds
                .ok_or_else(|| invalid_capture("capture surface has no screen bounds"))?;
            let captured = Arc::new((*captured).clone().with_screen_origin(ScreenPoint {
                x: origin.x,
                y: origin.y,
            }));
            surface.latest = Some(captured);
            updated = true;
        }
        Ok(updated)
    }

    /// 判断每个 surface 都已经至少交付过一张当前拓扑的帧。
    pub(super) fn has_complete_frame(&self) -> bool {
        !self.surfaces.is_empty() && self.surfaces.iter().all(|surface| surface.latest.is_some())
    }

    /// 将 primary 与 popup 最近帧按屏幕矩形合成为一个带屏幕原点的视觉帧。
    pub(super) fn compose(
        &self,
        frame_id: FrameId,
        topology_generation: TopologyGeneration,
        window: WindowIdentity,
    ) -> Result<Arc<CapturedFrame>, VisionError> {
        if !self.has_complete_frame() {
            return Err(invalid_capture(
                "capture surface set is missing a current frame",
            ));
        }
        let canvas = self
            .surfaces
            .iter()
            .filter_map(|surface| surface.entry.bounds)
            .reduce(PhysicalRect::union)
            .ok_or_else(|| invalid_capture("capture surface set has no screen bounds"))?;
        let primary = self
            .surfaces
            .iter()
            .find(|surface| surface.entry.role == WindowRole::Primary)
            .and_then(|surface| surface.latest.as_ref())
            .ok_or_else(|| invalid_capture("primary capture surface has no frame"))?;
        let row_bytes = canvas
            .width
            .checked_mul(4)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| invalid_capture("composite capture row byte length overflow"))?;
        let pixel_bytes = row_bytes
            .checked_mul(canvas.height as usize)
            .ok_or_else(|| invalid_capture("composite capture pixel byte length overflow"))?;
        let mut pixels = vec![0_u8; pixel_bytes];
        for surface in &self.surfaces {
            let bounds = surface
                .entry
                .bounds
                .ok_or_else(|| invalid_capture("capture surface has no screen bounds"))?;
            let source = surface
                .latest
                .as_ref()
                .ok_or_else(|| invalid_capture("capture surface has no latest frame"))?;
            blit_surface(&mut pixels, row_bytes, canvas, bounds, source);
        }
        CapturedFrame::from_bgra8(
            frame_id,
            topology_generation,
            window,
            primary.timestamp_qpc,
            canvas.width,
            canvas.height,
            primary.dpi_x,
            primary.dpi_y,
            row_bytes,
            pixels,
        )
        .map(|frame| {
            Arc::new(frame.with_screen_origin(ScreenPoint {
                x: canvas.x,
                y: canvas.y,
            }))
        })
    }
}

impl CaptureSurface {
    /// 为一个有效窗口建立独立 WGC item、frame pool 和 session。
    fn new(
        entry: WindowTopologyEntry,
        graphics: Arc<GraphicsDevice>,
        notify: Arc<Notify>,
        frame_pool_size: i32,
        include_cursor: bool,
        max_dimension: Option<u32>,
    ) -> Result<Self, VisionError> {
        let hwnd = native_window(entry.identity.handle);
        validate_window(hwnd, entry.identity)?;
        let item = create_capture_item(hwnd)?;
        let size = item
            .Size()
            .map_err(|error| capture_error("failed to query capture item size", error))?;
        let (width, height) = valid_size(size)?;
        if let Some(max_dimension) = max_dimension {
            if width.max(height) > max_dimension {
                return Err(invalid_capture(format!(
                    "capture item size {width}x{height} exceeds max dimension {max_dimension}"
                )));
            }
        }
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &graphics.capture_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            frame_pool_size,
            size,
        )
        .map_err(|error| capture_error("failed to create WGC frame pool", error))?;
        let session = pool
            .CreateCaptureSession(&item)
            .map_err(|error| capture_error("failed to create WGC capture session", error))?;
        session
            .SetIsCursorCaptureEnabled(include_cursor)
            .map_err(|error| capture_error("failed to configure cursor capture", error))?;
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
        Ok(Self {
            entry,
            pool,
            session,
            size,
            frame_pool_size,
            latest: None,
        })
    }

    /// 处理单个 popup resize，保证后续帧不继续使用旧 pool 尺寸。
    fn recreate_if_resized(
        &mut self,
        frame: &CapturedFrame,
        readback: &mut ReadbackState,
        graphics: &GraphicsDevice,
    ) -> Result<(), VisionError> {
        let width = i32::try_from(frame.width)
            .map_err(|_| invalid_capture("captured frame width does not fit WinRT size"))?;
        let height = i32::try_from(frame.height)
            .map_err(|_| invalid_capture("captured frame height does not fit WinRT size"))?;
        if self.size.Width == width && self.size.Height == height {
            return Ok(());
        }
        let new_size = SizeInt32 {
            Width: width,
            Height: height,
        };
        self.pool
            .Recreate(
                &graphics.capture_device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                self.frame_pool_size,
                new_size,
            )
            .map_err(|error| capture_error("failed to recreate resized WGC frame pool", error))?;
        self.size = new_size;
        readback.clear();
        self.latest = None;
        Ok(())
    }
}

impl Drop for CaptureSurface {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

/// 过滤为实际捕获的 primary + owned popup，并要求每个窗口都有可映射矩形。
fn capture_entries(topology: &WindowTopology) -> Result<Vec<WindowTopologyEntry>, VisionError> {
    let mut entries = topology
        .windows
        .iter()
        .filter(|entry| matches!(entry.role, WindowRole::Primary | WindowRole::OwnedPopup))
        .cloned()
        .collect::<Vec<_>>();
    if entries.iter().any(|entry| entry.bounds.is_none()) {
        return Err(invalid_capture(
            "capture surface topology contains a window without bounds",
        ));
    }
    entries.sort_by_key(|entry| {
        (
            if entry.role == WindowRole::Primary {
                0_u8
            } else {
                1_u8
            },
            entry.identity.handle,
        )
    });
    if entries
        .first()
        .is_none_or(|entry| entry.role != WindowRole::Primary)
    {
        return Err(invalid_capture(
            "capture surface topology has no primary window",
        ));
    }
    Ok(entries)
}

/// 将 source surface 缩放到其当前屏幕矩形并 alpha 合成到 canvas。
fn blit_surface(
    destination: &mut [u8],
    destination_stride: usize,
    canvas: PhysicalRect,
    target: PhysicalRect,
    source: &CapturedFrame,
) {
    let offset_x = (i64::from(target.x) - i64::from(canvas.x)) as u32;
    let offset_y = (i64::from(target.y) - i64::from(canvas.y)) as u32;
    for destination_y in 0..target.height {
        let source_y =
            (u64::from(destination_y) * u64::from(source.height) / u64::from(target.height)) as u32;
        for destination_x in 0..target.width {
            let source_x = (u64::from(destination_x) * u64::from(source.width)
                / u64::from(target.width)) as u32;
            let Some(pixel) = source.pixel(source_x, source_y) else {
                continue;
            };
            let x = offset_x + destination_x;
            let y = offset_y + destination_y;
            let offset = y as usize * destination_stride + x as usize * 4;
            if offset + 4 > destination.len() {
                continue;
            }
            blend_pixel(&mut destination[offset..offset + 4], pixel);
        }
    }
}

/// 对 popup 的透明像素做预乘前的简单 alpha 合成，避免覆盖 primary 内容。
fn blend_pixel(destination: &mut [u8], source: [u8; 4]) {
    let alpha = u32::from(source[3]);
    if alpha == 0 {
        return;
    }
    if alpha == u32::from(u8::MAX) {
        destination.copy_from_slice(&source);
        return;
    }
    let inverse = u32::from(u8::MAX) - alpha;
    for channel in 0..3 {
        destination[channel] = ((u32::from(source[channel]) * alpha
            + u32::from(destination[channel]) * inverse)
            / u32::from(u8::MAX)) as u8;
    }
    destination[3] = (alpha + u32::from(destination[3]) * inverse / u32::from(u8::MAX)) as u8;
}

/// 从 WinRT 激活工厂取得 HWND 专用 GraphicsCaptureItem。
fn create_capture_item(hwnd: HWND) -> Result<GraphicsCaptureItem, VisionError> {
    let class_name = HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureItem");
    let interop: IGraphicsCaptureItemInterop = unsafe { RoGetActivationFactory(&class_name) }
        .map_err(|error| capture_error("failed to get GraphicsCaptureItem factory", error))?;
    unsafe { interop.CreateForWindow(hwnd) }
        .map_err(|error| capture_error("failed to create GraphicsCaptureItem for HWND", error))
}

/// 只接受仍指向原 PID 的 HWND，防止 popup 句柄复用到其它应用。
fn validate_window(hwnd: HWND, expected: WindowIdentity) -> Result<(), VisionError> {
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return Err(VisionError::WindowIdentityChanged {
            expected,
            actual: None,
        });
    }
    let mut process_id = 0_u32;
    // SAFETY: process_id 是同步调用期间的独占输出，hwnd 已由 IsWindow 校验。
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

/// 把领域层不透明 HWND 表示恢复为 Windows 类型。
fn native_window(handle: u64) -> HWND {
    HWND(handle as usize as *mut std::ffi::c_void)
}
