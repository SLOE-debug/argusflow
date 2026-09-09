//! WGC 与 DXGI 共用三槽异步区域读回；完成检查立即返回。
use super::{
    device::GraphicsDevice,
    error::{capture_error, invalid_capture},
    readback_queue::ReadbackQueue,
};
use argusflow_capture::{CapturedFrame, FrameId, QpcTimestamp, TopologyGeneration};
use argusflow_core::{CaptureError, WindowIdentity};
use std::sync::Arc;
use windows::{
    Graphics::Capture::Direct3D11CaptureFrame,
    Win32::{
        Graphics::Direct3D11::{D3D11_BOX, D3D11_TEXTURE2D_DESC, ID3D11Texture2D},
        System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess,
    },
    core::Interface,
};

#[derive(Debug)]
struct FrameMetadata {
    window: WindowIdentity,
    frame_id: FrameId,
    generation: TopologyGeneration,
    timestamp: QpcTimestamp,
    width: u32,
    height: u32,
    dpi: u32,
}

/// 槽位由捕获线程拥有，消费者不接触 GPU 资源。
#[derive(Debug, Default)]
pub(super) struct ReadbackState {
    queue: ReadbackQueue<FrameMetadata>,
}

impl ReadbackState {
    pub(super) fn clear(&mut self) {
        self.queue = ReadbackQueue::default();
    }
    pub(super) fn available(&self) -> bool {
        self.queue.available()
    }
    /// 冻结最早完成版本，保留提交时的身份与拓扑。
    pub(super) fn poll(
        &mut self,
        graphics: &GraphicsDevice,
    ) -> Result<Option<Arc<CapturedFrame>>, CaptureError> {
        let Some((metadata, mut regions)) = self
            .queue
            .poll(&graphics.context)
            .map_err(|_| invalid_capture("WGC readback failed"))?
        else {
            return Ok(None);
        };
        let pixels = regions
            .pop()
            .ok_or_else(|| invalid_capture("WGC readback has no pixels"))?
            .pixels;
        CapturedFrame::from_bgra8(
            metadata.frame_id,
            metadata.generation,
            metadata.window,
            metadata.timestamp,
            metadata.width,
            metadata.height,
            metadata.dpi,
            metadata.dpi,
            metadata.width as usize * 4,
            pixels,
        )
        .map(|frame| Some(Arc::new(frame)))
    }
}

/// 只提交 GPU 复制，随后立即关闭 WGC 帧租约。
pub(super) fn submit_frame(
    frame: &Direct3D11CaptureFrame,
    graphics: &GraphicsDevice,
    readback: &mut ReadbackState,
    window: WindowIdentity,
    frame_id: FrameId,
    generation: TopologyGeneration,
) -> Result<(), CaptureError> {
    let surface = frame
        .Surface()
        .map_err(|error| capture_error("failed to get WGC surface", error))?;
    let access: IDirect3DDxgiInterfaceAccess = surface
        .cast()
        .map_err(|error| capture_error("failed to access WGC surface", error))?;
    // SAFETY: WGC surface 拥有纹理，生命周期覆盖 GPU 命令提交。
    let texture: ID3D11Texture2D = unsafe { access.GetInterface() }
        .map_err(|error| capture_error("failed to get WGC texture", error))?;
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe {
        texture.GetDesc(&mut desc);
    }
    if desc.Format != windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM
        || desc.SampleDesc.Count != 1
    {
        return Err(invalid_capture("invalid WGC pixel format"));
    }
    let timestamp = frame
        .SystemRelativeTime()
        .map_err(|error| capture_error("failed to read WGC timestamp", error))?;
    let metadata = FrameMetadata {
        window,
        frame_id,
        generation,
        timestamp: QpcTimestamp::new(timestamp.Duration.max(0) as u64),
        width: desc.Width,
        height: desc.Height,
        dpi: super::dpi::window_dpi(super::window_identity::native_window(window.handle)),
    };
    readback
        .queue
        .submit(
            &graphics.device,
            &graphics.context,
            &texture,
            vec![D3D11_BOX {
                left: 0,
                top: 0,
                front: 0,
                right: desc.Width,
                bottom: desc.Height,
                back: 1,
            }],
            metadata,
        )
        .map_err(|_| invalid_capture("failed to submit WGC readback"))
}
