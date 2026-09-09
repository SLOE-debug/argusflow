//! 按需 staging 读回，映射成功才分配输出像素并按租约计费。
use super::{Graphics, Texture, failure, invalid};
use argusflow_capture_contracts::{ByteBudget, CaptureResult, PixelFormat, PixelImage};
use std::{sync::Arc, time::Instant};
use windows::Win32::Graphics::{Direct3D11::*, Dxgi::DXGI_ERROR_WAS_STILL_DRAWING};
pub(in crate::capture) struct PendingRead {
    pub staging: Arc<Texture>,
    _source: Arc<Texture>,
    pub started: Instant,
}
impl PendingRead {
    pub fn new(
        graphics: &Graphics,
        source: Arc<Texture>,
        reused: Option<Arc<Texture>>,
    ) -> CaptureResult<Self> {
        let staging = match reused {
            Some(texture) if texture.width == source.width && texture.height == source.height => {
                texture
            }
            _ => Arc::new(Texture::new(graphics, source.width, source.height, true)?),
        };
        // SAFETY: 源和目标同尺寸、格式，目标只在本请求完成后归还资源池。
        unsafe {
            graphics
                .context
                .CopyResource(&staging.native, &source.native)
        };
        Ok(Self {
            staging,
            _source: source,
            started: Instant::now(),
        })
    }
    pub fn poll(&self, graphics: &Graphics, cpu: &ByteBudget) -> CaptureResult<Option<PixelImage>> {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: staging 具备 CPU_READ，非阻塞映射不会阻塞采集循环。
        match unsafe {
            graphics.context.Map(
                &self.staging.native,
                0,
                D3D11_MAP_READ,
                D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                Some(&mut mapped),
            )
        } {
            Err(error) if error.code() == DXGI_ERROR_WAS_STILL_DRAWING => return Ok(None),
            Err(error) => return Err(failure(error)),
            Ok(()) => {}
        }
        let result = (|| {
            let row = self.staging.width as usize * 4;
            let height = self.staging.height as usize;
            if mapped.pData.is_null() || (mapped.RowPitch as usize) < row {
                return Err(invalid("invalid image mapping"));
            }
            let reservation = cpu.reserve(row * height)?;
            let mut output = vec![0; row * height];
            for y in 0..height {
                // SAFETY: 每行仅借用宽度对应字节，不读取驱动填充区域。
                let source = unsafe {
                    std::slice::from_raw_parts(
                        mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize),
                        row,
                    )
                };
                output[y * row..(y + 1) * row].copy_from_slice(source);
            }
            PixelImage::new(
                self.staging.width,
                self.staging.height,
                row,
                PixelFormat::Bgrx8,
                output,
                reservation,
            )
        })();
        // SAFETY: 与成功 Map 一一对应，预算失败也必须解除映射。
        unsafe { graphics.context.Unmap(&self.staging.native, 0) };
        result.map(Some)
    }
}
