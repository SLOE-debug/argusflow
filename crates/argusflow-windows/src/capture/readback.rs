//! D3D11 staging texture 的订阅级复用与 GPU readback。

use std::{
    ffi::c_void,
    sync::Arc,
    time::{Duration, Instant},
};

use argusflow_core::WindowIdentity;
use argusflow_vision::{CapturedFrame, FrameId, QpcTimestamp, TopologyGeneration, VisionError};
use windows::Graphics::{Capture::Direct3D11CaptureFrame, DirectX::Direct3D11::IDirect3DSurface};
use windows::Win32::Graphics::{
    Direct3D11::{
        D3D11_CPU_ACCESS_READ, D3D11_MAP_FLAG_DO_NOT_WAIT, D3D11_MAP_READ,
        D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, ID3D11Resource,
        ID3D11Texture2D,
    },
    Dxgi::{
        Common::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
        DXGI_ERROR_WAS_STILL_DRAWING,
    },
};
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::core::Interface;

use super::{
    device::GraphicsDevice,
    error::{capture_error, invalid_capture},
};

/// 一个 capture subscription 复用同尺寸的 CPU-readable staging texture。
#[derive(Debug, Default)]
pub(super) struct ReadbackState {
    /// 当前尺寸可复用的 staging texture。
    staging: Option<StagingTexture>,
}

impl ReadbackState {
    /// 在 frame pool resize 后丢弃旧尺寸的 staging resource。
    pub(super) fn clear(&mut self) {
        self.staging = None;
    }
}

/// 把 WGC GPU surface 复制到订阅级 staging texture，并复制成短期拥有型 BGRA8 buffer。
pub(super) fn readback_frame(
    frame: &Direct3D11CaptureFrame,
    graphics: &GraphicsDevice,
    readback: &mut ReadbackState,
    window: WindowIdentity,
    frame_id: FrameId,
    topology_generation: TopologyGeneration,
    deadline: Instant,
    timeout: Duration,
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
    let staging = staging_texture(graphics, readback, &source_desc)?;
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
    // CopyResource 只排队 GPU 工作；Flush 后使用 DO_NOT_WAIT 轮询，避免 Map 无限阻塞 Tokio worker。
    unsafe { graphics.context.Flush() };
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    loop {
        if Instant::now() >= deadline {
            return Err(readback_timeout(timeout));
        }
        // SAFETY: staging texture 使用 CPU_READ，且 immediate context 由 readback mutex 独占。
        let mapped_result = unsafe {
            graphics.context.Map(
                &staging_resource,
                0,
                D3D11_MAP_READ,
                D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                Some(&mut mapped),
            )
        };
        match mapped_result {
            Ok(()) => break,
            Err(error) if error.code() == DXGI_ERROR_WAS_STILL_DRAWING => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                std::thread::sleep(remaining.min(Duration::from_millis(1)));
            }
            Err(error) => {
                return Err(capture_error("failed to map D3D11 staging texture", error));
            }
        }
    }
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
                let dpi = super::dpi::window_dpi(native_window(window.handle));
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

/// 把 GPU readback 超过当前捕获预算映射为统一的帧超时。
fn readback_timeout(timeout: Duration) -> VisionError {
    VisionError::FrameTimeout {
        timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
    }
}

/// 记录 staging texture 的创建参数，窗口 resize 后强制替换。
#[derive(Debug)]
struct StagingTexture {
    /// D3D11 staging resource。
    texture: ID3D11Texture2D,
    /// 创建时的宽度。
    width: u32,
    /// 创建时的高度。
    height: u32,
    /// 创建时的像素格式。
    format: DXGI_FORMAT,
}

/// 取得与 source texture 尺寸匹配的 staging texture，steady capture 不重复创建设备资源。
pub(super) fn staging_texture(
    graphics: &GraphicsDevice,
    readback: &mut ReadbackState,
    source_desc: &D3D11_TEXTURE2D_DESC,
) -> Result<ID3D11Texture2D, VisionError> {
    let reusable = readback.staging.as_ref().is_some_and(|staging| {
        staging.width == source_desc.Width
            && staging.height == source_desc.Height
            && staging.format == source_desc.Format
    });
    if !reusable {
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
        let texture =
            staging.ok_or_else(|| invalid_capture("D3D11 returned no staging texture"))?;
        readback.staging = Some(StagingTexture {
            texture,
            width: source_desc.Width,
            height: source_desc.Height,
            format: source_desc.Format,
        });
    }
    readback
        .staging
        .as_ref()
        .map(|staging| staging.texture.clone())
        .ok_or_else(|| invalid_capture("D3D11 staging texture was not installed"))
}

/// 把领域层的不透明 HWND 表示恢复为 Windows 类型。
fn native_window(handle: u64) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(handle as usize as *mut c_void)
}
