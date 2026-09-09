//! 差分只读回块边界与计数，原始图像始终留在 GPU。
use super::{Buffer, Graphics, Texture, failure, invalid};
use argusflow_capture_contracts::{CaptureResult, PixelChanges, PixelRect};
use std::time::Instant;
use windows::{
    Win32::Graphics::{
        Direct3D::Fxc::D3DCompile, Direct3D11::*, Dxgi::DXGI_ERROR_WAS_STILL_DRAWING,
    },
    core::s,
};
pub(in crate::capture) struct Difference {
    shader: ID3D11ComputeShader,
}
impl Difference {
    pub fn new(device: &ID3D11Device) -> CaptureResult<Self> {
        let source = include_bytes!("difference.hlsl");
        let mut code = None;
        let mut errors = None;
        // SAFETY: 编译内嵌着色器，无外部 include 或运行时输入。
        unsafe {
            D3DCompile(
                source.as_ptr().cast(),
                source.len(),
                s!("difference.hlsl"),
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
        let code = code.ok_or_else(|| invalid("missing shader bytecode"))?;
        let mut shader = None;
        // SAFETY: blob 覆盖整个字节码，并在调用期间存活。
        unsafe {
            device.CreateComputeShader(
                std::slice::from_raw_parts(code.GetBufferPointer().cast(), code.GetBufferSize()),
                None,
                Some(&mut shader),
            )
        }
        .map_err(failure)?;
        Ok(Self {
            shader: shader.ok_or_else(|| invalid("missing shader"))?,
        })
    }
    pub fn submit(
        &self,
        graphics: &Graphics,
        before: &Texture,
        after: &Texture,
        locations: &[[u32; 4]],
    ) -> CaptureResult<PendingDifference> {
        if locations.is_empty() || locations.len() > 65535 {
            return Err(invalid("difference tile count"));
        }
        let data: Vec<_> = locations
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let input = Buffer::new(
            graphics,
            data.len() as u32,
            16,
            D3D11_BIND_SHADER_RESOURCE,
            false,
            Some(&data),
        )?;
        let output = Buffer::new(
            graphics,
            locations.len() as u32 * 32,
            16,
            D3D11_BIND_UNORDERED_ACCESS,
            false,
            None,
        )?;
        let staging = Buffer::new(
            graphics,
            locations.len() as u32 * 32,
            0,
            D3D11_BIND_FLAG(0),
            true,
            None,
        )?;
        let geometry: Vec<_> = [before.width, before.height, locations.len() as u32, 0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let constants = Buffer::new(
            graphics,
            16,
            0,
            D3D11_BIND_CONSTANT_BUFFER,
            false,
            Some(&geometry),
        )?;
        let mut positions = None;
        let mut result = None;
        // SAFETY: 全部资源同设备，绑定和解除绑定均由本线程独占 context。
        unsafe {
            graphics
                .device
                .CreateShaderResourceView(&input.native, None, Some(&mut positions))
                .map_err(failure)?;
            graphics
                .device
                .CreateUnorderedAccessView(&output.native, None, Some(&mut result))
                .map_err(failure)?;
            graphics.context.CSSetShader(&self.shader, None);
            graphics.context.CSSetShaderResources(
                0,
                Some(&[
                    Some(before.view(graphics)?),
                    Some(after.view(graphics)?),
                    positions,
                ]),
            );
            graphics
                .context
                .CSSetConstantBuffers(0, Some(&[Some(constants.native.clone())]));
            graphics
                .context
                .CSSetUnorderedAccessViews(0, 1, Some([result].as_ptr()), None);
            graphics.context.Dispatch(locations.len() as u32, 1, 1);
            graphics
                .context
                .CSSetUnorderedAccessViews(0, 1, Some([None].as_ptr()), None);
            graphics
                .context
                .CSSetShaderResources(0, Some(&[None, None, None]));
            graphics.context.CSSetConstantBuffers(0, Some(&[None]));
            graphics
                .context
                .CopyResource(&staging.native, &output.native);
        }
        Ok(PendingDifference {
            staging,
            _inputs: vec![input, output, constants],
            count: locations.len(),
            started: Instant::now(),
        })
    }
}
pub(in crate::capture) struct PendingDifference {
    staging: Buffer,
    _inputs: Vec<Buffer>,
    count: usize,
    pub started: Instant,
}
impl PendingDifference {
    pub fn poll(&self, graphics: &Graphics) -> CaptureResult<Option<(PixelChanges, Vec<bool>)>> {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: staging 为 CPU_READ；DO_NOT_WAIT 使采集线程不等待 GPU。
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
            if mapped.pData.is_null() {
                return Err(invalid("null difference mapping"));
            }
            // SAFETY: 成功映射的 buffer 大小固定为 count*32，且本作用域内不会解除映射。
            let values =
                unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u32>(), self.count * 8) };
            let mut changes = PixelChanges::default();
            let mut changed = Vec::with_capacity(self.count);
            for values in values.as_chunks::<8>().0 {
                changed.push(values[4] != 0);
                changes.changed_pixels += u64::from(values[4]);
                changes.compared_pixels += u64::from(values[5]);
                if values[4] != 0 {
                    changes.regions.push(PixelRect::new(
                        values[0],
                        values[1],
                        values[2] - values[0],
                        values[3] - values[1],
                    )?);
                }
            }
            Ok((changes, changed))
        })();
        // SAFETY: 与上面的成功 Map 一一对应，错误结果也解除映射。
        unsafe { graphics.context.Unmap(&self.staging.native, 0) };
        result.map(Some)
    }
    pub fn bytes(&self) -> u64 {
        self.count as u64 * 32
    }
}
