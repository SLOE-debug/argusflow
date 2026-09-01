//! 单 HWND 的 WGC session、frame pool 与 CPU readback。

use std::{
    mem::size_of,
    sync::Arc,
    time::{Duration, Instant},
};

use argusflow_core::{ScreenPoint, WindowIdentity};
use argusflow_vision::{CapturedFrame, FrameId, PhysicalRect, TopologyGeneration, VisionError};
use tokio::sync::Notify;
use windows::{
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::{
            Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem,
            GraphicsCaptureSession,
        },
        DirectX::DirectXPixelFormat,
        SizeInt32,
    },
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
        System::WinRT::{Graphics::Capture::IGraphicsCaptureItemInterop, RoGetActivationFactory},
    },
    core::{HSTRING, IInspectable},
};

use super::{
    device::GraphicsDevice,
    error::{capture_error, invalid_capture},
    readback::{ReadbackState, readback_frame},
    window_identity::{native_window, validate_window},
};

/// 一个窗口实例对应且只对应一个 WGC 捕获面。
#[derive(Debug)]
pub(super) struct WindowCaptureSurface {
    /// 捕获期间固定的 HWND/PID。
    window: WindowIdentity,
    /// WGC frame pool。
    pool: Direct3D11CaptureFramePool,
    /// 保持捕获存活的 session。
    session: GraphicsCaptureSession,
    /// FrameArrived 事件令牌。
    frame_arrived_token: i64,
    /// 当前 frame pool 尺寸。
    size: SizeInt32,
    /// Recreate 时保持一致的缓冲数量。
    frame_pool_size: i32,
    /// 单边允许的最大像素数。
    max_dimension: Option<u32>,
}

impl WindowCaptureSurface {
    /// 为已经验证的窗口创建一个独立 WGC session。
    pub(super) fn new(
        window: WindowIdentity,
        graphics: &GraphicsDevice,
        notify: Arc<Notify>,
        frame_pool_size: i32,
        include_cursor: bool,
        max_dimension: Option<u32>,
    ) -> Result<Self, VisionError> {
        let hwnd = native_window(window.handle);
        validate_window(hwnd, window)?;
        let item = create_capture_item(hwnd)?;
        let size = item
            .Size()
            .map_err(|error| capture_error("failed to query WGC item size", error))?;
        validate_size(size, max_dimension)?;
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
        let frame_arrived_token = pool
            .FrameArrived(&handler)
            .map_err(|error| capture_error("failed to subscribe to WGC frames", error))?;
        if let Err(error) = session.StartCapture() {
            close_resources(&pool, &session, Some(frame_arrived_token));
            return Err(capture_error("failed to start WGC capture", error));
        }
        Ok(Self {
            window,
            pool,
            session,
            frame_arrived_token,
            size,
            frame_pool_size,
            max_dimension,
        })
    }

    /// 读取当前积压队列中最新的一张帧；resize 时重建 pool 并等待后续新尺寸帧。
    ///
    /// WGC frame pool 是一个有界队列。工作流在 Delay 或 UI 输入期间不会持续消费它，
    /// 因此一次普通的 `TryGetNextFrame` 很可能拿到动作前的旧帧。场景读取关心的是“当前
    /// 画面”而非逐帧回放，所以必须先排空队列并只对最新帧执行昂贵的 GPU readback。
    pub(super) fn poll(
        &mut self,
        graphics: &GraphicsDevice,
        readback: &mut ReadbackState,
        frame_id: FrameId,
        generation: TopologyGeneration,
        deadline: Instant,
        timeout: Duration,
    ) -> Result<Option<Arc<CapturedFrame>>, VisionError> {
        let Some(frame) = self.take_latest_pending_frame()? else {
            return Ok(None);
        };
        let content_size = frame
            .ContentSize()
            .map_err(|error| capture_error("failed to query WGC content size", error))?;
        if content_size != self.size {
            frame
                .Close()
                .map_err(|error| capture_error("failed to close resized WGC frame", error))?;
            self.recreate(graphics, readback, content_size)?;
            return Ok(None);
        }
        let captured = readback_frame(
            &frame,
            graphics,
            readback,
            self.window,
            frame_id,
            generation,
            deadline,
            timeout,
        );
        let close_result = frame.Close();
        let captured = captured?;
        close_result.map_err(|error| capture_error("failed to close WGC frame", error))?;
        let bounds = window_bounds(native_window(self.window.handle))?;
        Ok(Some(Arc::new((*captured).clone().with_screen_origin(
            ScreenPoint {
                x: bounds.x,
                y: bounds.y,
            },
        ))))
    }

    /// 排空 WGC 的有界待处理队列，并确定性关闭被更新帧替代的旧资源。
    fn take_latest_pending_frame(&self) -> Result<Option<Direct3D11CaptureFrame>, VisionError> {
        drain_latest_available(
            || self.pool.TryGetNextFrame().ok(),
            |frame| {
                frame
                    .Close()
                    .map_err(|error| capture_error("failed to close superseded WGC frame", error))
            },
        )
    }

    /// 返回当前 DWM 可见物理边界，供调用方维护窗口 generation。
    pub(super) fn bounds(&self) -> Result<PhysicalRect, VisionError> {
        window_bounds(native_window(self.window.handle))
    }

    /// 按 WGC 新尺寸重建 frame pool 并丢弃旧 staging texture。
    fn recreate(
        &mut self,
        graphics: &GraphicsDevice,
        readback: &mut ReadbackState,
        size: SizeInt32,
    ) -> Result<(), VisionError> {
        validate_size(size, self.max_dimension)?;
        self.pool
            .Recreate(
                &graphics.capture_device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                self.frame_pool_size,
                size,
            )
            .map_err(|error| capture_error("failed to recreate WGC frame pool", error))?;
        self.size = size;
        readback.clear();
        Ok(())
    }
}

/// 从有界积压队列保留最新项目，并通过调用方提供的资源释放函数丢弃更旧项目。
fn drain_latest_available<T>(
    mut next: impl FnMut() -> Option<T>,
    mut discard: impl FnMut(T) -> Result<(), VisionError>,
) -> Result<Option<T>, VisionError> {
    let Some(mut latest) = next() else {
        return Ok(None);
    };
    while let Some(newer) = next() {
        discard(latest)?;
        latest = newer;
    }
    Ok(Some(latest))
}

impl Drop for WindowCaptureSurface {
    fn drop(&mut self) {
        close_resources(&self.pool, &self.session, Some(self.frame_arrived_token));
    }
}

/// 从 WinRT 激活工厂创建 HWND 专用捕获项。
fn create_capture_item(hwnd: HWND) -> Result<GraphicsCaptureItem, VisionError> {
    let class_name = HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureItem");
    let interop: IGraphicsCaptureItemInterop = unsafe { RoGetActivationFactory(&class_name) }
        .map_err(|error| capture_error("failed to get GraphicsCaptureItem factory", error))?;
    unsafe { interop.CreateForWindow(hwnd) }
        .map_err(|error| capture_error("failed to create GraphicsCaptureItem for HWND", error))
}

/// 校验 WGC item 的非空尺寸和显式资源上限。
fn validate_size(size: SizeInt32, max_dimension: Option<u32>) -> Result<(), VisionError> {
    let width =
        u32::try_from(size.Width).map_err(|_| invalid_capture("capture width is invalid"))?;
    let height =
        u32::try_from(size.Height).map_err(|_| invalid_capture("capture height is invalid"))?;
    if width == 0 || height == 0 {
        return Err(invalid_capture("capture item has an empty size"));
    }
    if max_dimension.is_some_and(|limit| width.max(height) > limit) {
        return Err(invalid_capture(format!(
            "capture item size {width}x{height} exceeds configured maximum"
        )));
    }
    Ok(())
}

/// 读取 DWM 可见物理边界，避免 GetWindowRect 的 DPI 虚拟化和透明 resize border。
fn window_bounds(hwnd: HWND) -> Result<PhysicalRect, VisionError> {
    let mut bounds = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut bounds as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
    }
    .map_err(|error| capture_error("failed to read DWM window bounds", error))?;
    let width = u32::try_from(bounds.right.saturating_sub(bounds.left))
        .map_err(|_| invalid_capture("window width is invalid"))?;
    let height = u32::try_from(bounds.bottom.saturating_sub(bounds.top))
        .map_err(|_| invalid_capture("window height is invalid"))?;
    PhysicalRect::new(bounds.left, bounds.top, width, height)
        .ok_or_else(|| invalid_capture("window bounds are empty"))
}

/// 在创建线程中按事件、session、pool 顺序释放 WGC 资源。
fn close_resources(
    pool: &Direct3D11CaptureFramePool,
    session: &GraphicsCaptureSession,
    token: Option<i64>,
) {
    if let Some(token) = token {
        let _ = pool.RemoveFrameArrived(token);
    }
    let _ = session.Close();
    let _ = pool.Close();
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;

    /// 场景捕获必须跳过动作前的积压帧，只把最新画面交给稳定门控和 OCR。
    #[test]
    fn pending_capture_frames_are_drained_to_the_latest_image() {
        let mut pending = VecDeque::from(["17:31", "17:31-caret", "19:44"]);
        let mut discarded = Vec::new();

        let latest = drain_latest_available(
            || pending.pop_front(),
            |frame| {
                discarded.push(frame);
                Ok(())
            },
        )
        .expect("pending frame drain should succeed");

        assert_eq!(latest, Some("19:44"));
        assert_eq!(discarded, ["17:31", "17:31-caret"]);
    }
}
