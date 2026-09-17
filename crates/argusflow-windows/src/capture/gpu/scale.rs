//! GPU 缩放和旋转完整桌面；只读回最终 1080P 缓冲，不遍历 dirty tiles。
use super::{Buffer, Graphics, Texture, failure, invalid};
use argusflow_capture_contracts::*;
use windows::{
    Win32::Graphics::{
        Direct3D::Fxc::D3DCompile, Direct3D11::*, Dxgi::DXGI_ERROR_WAS_STILL_DRAWING,
    },
    core::s,
};

pub(in crate::capture) struct Scaler {
    shader: ID3D11ComputeShader,
    input: Texture,
    output: Buffer,
    staging: Buffer,
    constants: Buffer,
    width: u32,
    height: u32,
}
impl Scaler {
    pub fn new(
        graphics: &Graphics,
        raw: (u32, u32),
        size: (u32, u32),
        rotation: Rotation,
    ) -> CaptureResult<Self> {
        let source = include_bytes!("scale.hlsl");
        let mut code = None;
        let mut errors = None;
        // SAFETY: 着色器来自仓库内固定内容，不解析外部脚本或 include。
        unsafe {
            D3DCompile(
                source.as_ptr().cast(),
                source.len(),
                s!("scale.hlsl"),
                None,
                None,
                s!("main"),
                s!("cs_5_0"),
                0,
                0,
                &mut code,
                Some(&mut errors),
            )
        }
        .map_err(failure)?;
        let code = code.ok_or_else(|| invalid("missing scale bytecode"))?;
        let mut shader = None;
        unsafe {
            graphics.device.CreateComputeShader(
                std::slice::from_raw_parts(code.GetBufferPointer().cast(), code.GetBufferSize()),
                None,
                Some(&mut shader),
            )
        }
        .map_err(failure)?;
        let rotation = match rotation {
            Rotation::Identity => 0,
            Rotation::Clockwise90 => 1,
            Rotation::Clockwise180 => 2,
            Rotation::Clockwise270 => 3,
        };
        let bytes = size
            .0
            .checked_mul(size.1)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| invalid("scale size overflow"))?;
        let constants: Vec<_> = [raw.0, raw.1, size.0, size.1, rotation, 0, 0, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        Ok(Self {
            shader: shader.ok_or_else(|| invalid("missing scale shader"))?,
            input: Texture::new(graphics, raw.0, raw.1, false)?,
            output: Buffer::new(graphics, bytes, 4, D3D11_BIND_UNORDERED_ACCESS, false, None)?,
            staging: Buffer::new(graphics, bytes, 0, D3D11_BIND_FLAG(0), true, None)?,
            constants: Buffer::new(
                graphics,
                32,
                0,
                D3D11_BIND_CONSTANT_BUFFER,
                false,
                Some(&constants),
            )?,
            width: size.0,
            height: size.1,
        })
    }
    pub fn submit(&self, graphics: &Graphics, source: &ID3D11Texture2D) -> CaptureResult<()> {
        let mut output = None;
        // SAFETY: 全部资源属于本 actor 的设备；上一帧已完成读回后才复用缓冲。
        unsafe {
            graphics.context.CopyResource(&self.input.native, source);
            graphics
                .device
                .CreateUnorderedAccessView(&self.output.native, None, Some(&mut output))
                .map_err(failure)?;
            graphics.context.CSSetShader(&self.shader, None);
            graphics
                .context
                .CSSetShaderResources(0, Some(&[Some(self.input.view(graphics)?)]));
            graphics
                .context
                .CSSetConstantBuffers(0, Some(&[Some(self.constants.native.clone())]));
            graphics
                .context
                .CSSetUnorderedAccessViews(0, 1, Some([output].as_ptr()), None);
            graphics
                .context
                .Dispatch(self.width.div_ceil(16), self.height.div_ceil(16), 1);
            graphics
                .context
                .CSSetUnorderedAccessViews(0, 1, Some([None].as_ptr()), None);
            graphics.context.CSSetShaderResources(0, Some(&[None]));
            graphics.context.CSSetConstantBuffers(0, Some(&[None]));
            graphics
                .context
                .CopyResource(&self.staging.native, &self.output.native);
            graphics.context.Flush();
        }
        Ok(())
    }
    pub fn byte_len(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }
    pub fn poll(&self, graphics: &Graphics, cpu: &ByteBudget) -> CaptureResult<Option<PixelImage>> {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: DO_NOT_WAIT 避免等待 GPU；成功 Map 在所有返回路径都解除。
        match unsafe {
            graphics.context.Map(
                &self.staging.native,
                0,
                D3D11_MAP_READ,
                D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                Some(&mut mapped),
            )
        } {
            Err(e) if e.code() == DXGI_ERROR_WAS_STILL_DRAWING => return Ok(None),
            Err(e) => return Err(failure(e)),
            Ok(()) => {}
        }
        let result = (|| {
            if mapped.pData.is_null() {
                return Err(invalid("null scale buffer"));
            }
            let reservation = cpu.reserve(self.byte_len())?;
            // SAFETY: staging 大小为 width*height*4，复制期间映射有效。
            let bytes =
                unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>(), self.byte_len()) }
                    .to_vec();
            PixelImage::new(
                self.width,
                self.height,
                self.width as usize * 4,
                PixelFormat::Bgrx8,
                bytes,
                reservation,
            )
        })();
        unsafe { graphics.context.Unmap(&self.staging.native, 0) };
        result.map(Some)
    }
}
