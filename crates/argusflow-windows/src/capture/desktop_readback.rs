//! 窗口交集的 staging 分配、GPU 到 CPU 传输和映射生命周期。

use super::desktop_device::DesktopDevice;
use argusflow_core::InspectionFailure;
use std::time::{Duration, Instant};
use windows::Win32::Graphics::{
    Direct3D11::*,
    Dxgi::{
        Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
        DXGI_ERROR_WAS_STILL_DRAWING,
    },
};

/// 只缓存一个窗口区域；GPU 桌面更新时必须 invalidate，不能返回未确认的旧内容。
#[derive(Default)]
pub(super) struct DesktopReadback {
    /// CPU 可读纹理，按裁切尺寸复用。
    texture: Option<ID3D11Texture2D>,
    /// 最近分配的物理像素尺寸。
    size: (u32, u32),
    /// 仅同区域且未失效才允许省去重复 GPU 传输。
    sampled_region: Option<D3D11_BOX>,
}

impl DesktopReadback {
    /// 只撤销像素有效性，不丢弃可复用的原生分配。
    pub(super) fn invalidate(&mut self) {
        self.sampled_region = None;
    }

    /// 输入区域已在桌面尺寸内校验；闭包不能持有映射切片超过本次调用。
    pub(super) fn read<T>(
        &mut self,
        graphics: &DesktopDevice,
        desktop: &ID3D11Texture2D,
        region: &D3D11_BOX,
        consume: impl FnOnce(&[u8], usize, u32, u32) -> Result<T, InspectionFailure>,
    ) -> Result<T, InspectionFailure> {
        let width = region.right - region.left;
        let height = region.bottom - region.top;
        self.allocate(graphics, width, height)?;
        let staging = self
            .texture
            .as_ref()
            .ok_or(InspectionFailure::Unavailable)?;
        if self.sampled_region.as_ref() != Some(region) {
            // SAFETY: 源区域由调用方验证，目标尺寸恰好等于交集，目标起点为原点。
            unsafe {
                graphics.context.CopySubresourceRegion(
                    staging,
                    0,
                    0,
                    0,
                    0,
                    desktop,
                    0,
                    Some(region),
                )
            };
        }
        let mapped = map(graphics, staging)?;
        let _mapping = MappedTexture {
            graphics,
            texture: staging,
        };
        let stride = mapped.RowPitch as usize;
        if mapped.pData.is_null() || stride < width as usize * 4 {
            return Err(InspectionFailure::Unavailable);
        }
        let length = stride
            .checked_mul(height as usize)
            .ok_or(InspectionFailure::InvalidGeometry)?;
        // SAFETY: Map 成功且行跨度已核对；借用只存在于 consume，守卫随后 Unmap。
        let pixels = unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>(), length) };
        let result = consume(pixels, stride, width, height);
        if result.is_ok() {
            self.sampled_region = Some(*region);
        }
        result
    }

    /// 缩放后先释放旧区域，再按新尺寸创建；移动不会重新分配。
    fn allocate(
        &mut self,
        graphics: &DesktopDevice,
        width: u32,
        height: u32,
    ) -> Result<(), InspectionFailure> {
        if self.size == (width, height) && self.texture.is_some() {
            return Ok(());
        }
        self.texture = None;
        self.invalidate();
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..Default::default()
        };
        // SAFETY: 正尺寸区域由上层验证，输出槽和设备在调用期间有效。
        unsafe {
            graphics
                .device
                .CreateTexture2D(&desc, None, Some(&mut self.texture))
        }
        .map_err(|_| InspectionFailure::Unavailable)?;
        self.size = (width, height);
        Ok(())
    }
}

/// 提交后有界轮询，避免阻塞 Map 在 GPU 故障时无限卡住事件摄入。
fn map(
    graphics: &DesktopDevice,
    texture: &ID3D11Texture2D,
) -> Result<D3D11_MAPPED_SUBRESOURCE, InspectionFailure> {
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { graphics.context.Flush() };
    let deadline = Instant::now() + Duration::from_millis(100);
    loop {
        // SAFETY: texture 以 CPU_READ 创建，immediate context 的访问由外层 Mutex 串行化。
        let result = unsafe {
            graphics.context.Map(
                texture,
                0,
                D3D11_MAP_READ,
                D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                Some(&mut mapped),
            )
        };
        match result {
            Ok(()) => return Ok(mapped),
            Err(error) if error.code() == DXGI_ERROR_WAS_STILL_DRAWING => {
                if Instant::now() >= deadline {
                    return Err(InspectionFailure::Timeout);
                }
                std::thread::yield_now();
            }
            Err(_) => return Err(InspectionFailure::Unavailable),
        }
    }
}

/// 闭包返回错误或 panic 都必须解除已成功的映射。
struct MappedTexture<'a> {
    /// Map 所属的串行设备上下文。
    graphics: &'a DesktopDevice,
    /// 当前已映射的子资源 0。
    texture: &'a ID3D11Texture2D,
}
impl Drop for MappedTexture<'_> {
    fn drop(&mut self) {
        // SAFETY: 与构造前成功的 Map 一一配对。
        unsafe { self.graphics.context.Unmap(self.texture, 0) };
    }
}
