//! 停止录制后的硬件计算着色器差分；独占 D3D11 context，不阻塞采集设备。
use argusflow_core::{
    CaptureError, EvidenceFrame, EvidencePixelFormat, InspectionRect,
    capture::refinement::PixelDiffer,
};
use windows::{
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0, Fxc::D3DCompile},
            Direct3D11::*,
        },
    },
    core::s,
};

pub(super) struct GpuDifference {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    shader: ID3D11ComputeShader,
}

fn failure(error: impl std::fmt::Display) -> CaptureError {
    CaptureError::CaptureUnavailable {
        message: format!("GPU pixel difference: {error}"),
    }
}

impl GpuDifference {
    pub(super) fn new() -> Result<Self, CaptureError> {
        let mut device = None;
        let mut context = None;
        // SAFETY: 输出槽独占，明确使用硬件 FL11 设备，不建立软件兼容路径。
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG(0),
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        }
        .map_err(failure)?;
        let device = device.ok_or_else(|| failure("missing device"))?;
        let context = context.ok_or_else(|| failure("missing context"))?;
        let source = include_bytes!("gpu_difference.hlsl");
        let mut code = None;
        let mut errors = None;
        // SAFETY: 内嵌着色器及长度有效，无 include handler，blob 生命周期覆盖创建调用。
        unsafe {
            D3DCompile(
                source.as_ptr().cast(),
                source.len(),
                s!("gpu_difference.hlsl"),
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
        let code = code.ok_or_else(|| failure("missing shader bytecode"))?;
        let mut shader = None;
        unsafe {
            device.CreateComputeShader(
                std::slice::from_raw_parts(code.GetBufferPointer().cast(), code.GetBufferSize()),
                None,
                Some(&mut shader),
            )
        }
        .map_err(failure)?;
        Ok(Self {
            device,
            context,
            shader: shader.ok_or_else(|| failure("missing shader"))?,
        })
    }

    fn buffer(
        &self,
        bytes: u32,
        stride: u32,
        bind: D3D11_BIND_FLAG,
        usage: D3D11_USAGE,
        initial: Option<&[u8]>,
    ) -> Result<ID3D11Buffer, CaptureError> {
        let descriptor = D3D11_BUFFER_DESC {
            ByteWidth: bytes,
            Usage: usage,
            BindFlags: bind.0 as u32,
            CPUAccessFlags: if usage == D3D11_USAGE_STAGING {
                D3D11_CPU_ACCESS_READ.0 as u32
            } else {
                0
            },
            MiscFlags: if stride != 0 && usage != D3D11_USAGE_STAGING {
                D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32
            } else {
                0
            },
            StructureByteStride: stride,
        };
        let data = initial.map(|pixels| D3D11_SUBRESOURCE_DATA {
            pSysMem: pixels.as_ptr().cast(),
            ..Default::default()
        });
        let mut buffer = None;
        // SAFETY: 输入切片覆盖 ByteWidth，D3D11 在返回之前消费初始化数据。
        unsafe {
            self.device.CreateBuffer(
                &descriptor,
                data.as_ref().map(|data| data as *const _),
                Some(&mut buffer),
            )
        }
        .map_err(failure)?;
        buffer.ok_or_else(|| failure("missing buffer"))
    }
}

impl PixelDiffer for GpuDifference {
    fn compare(
        &mut self,
        previous: &EvidenceFrame,
        current: &EvidenceFrame,
    ) -> Result<Vec<InspectionRect>, CaptureError> {
        if previous.bounds() != current.bounds()
            || previous.format() != current.format()
            || previous.width() != current.width()
            || previous.height() != current.height()
        {
            return Err(failure("incompatible regions"));
        }
        let bytes = u32::try_from(current.pixels().len()).map_err(failure)?;
        if bytes > 128 * 1024 * 1024 {
            return Err(failure("region exceeds GPU budget"));
        }
        let columns = current.width().div_ceil(32);
        let rows = current.height().div_ceil(32);
        let output_bytes = columns
            .checked_mul(rows)
            .and_then(|tiles| tiles.checked_mul(16))
            .ok_or_else(|| failure("tile count overflow"))?;
        let before = self.buffer(
            bytes,
            4,
            D3D11_BIND_SHADER_RESOURCE,
            D3D11_USAGE_IMMUTABLE,
            Some(previous.pixels()),
        )?;
        let after = self.buffer(
            bytes,
            4,
            D3D11_BIND_SHADER_RESOURCE,
            D3D11_USAGE_IMMUTABLE,
            Some(current.pixels()),
        )?;
        let output = self.buffer(
            output_bytes,
            16,
            D3D11_BIND_UNORDERED_ACCESS,
            D3D11_USAGE_DEFAULT,
            None,
        )?;
        let staging = self.buffer(
            output_bytes,
            0,
            D3D11_BIND_FLAG(0),
            D3D11_USAGE_STAGING,
            None,
        )?;
        let geometry = [
            current.width(),
            current.height(),
            columns,
            match current.format() {
                EvidencePixelFormat::Bgrx8 => 0x00ff_ffff,
                EvidencePixelFormat::Rgba8 => u32::MAX,
            },
        ];
        let geometry_bytes: Vec<_> = geometry.into_iter().flat_map(u32::to_le_bytes).collect();
        let constants = self.buffer(
            16,
            0,
            D3D11_BIND_CONSTANT_BUFFER,
            D3D11_USAGE_IMMUTABLE,
            Some(&geometry_bytes),
        )?;
        let (mut before_view, mut after_view, mut output_view) = (None, None, None);
        // SAFETY: buffers 与本设备同源且具备对应绑定标志；所有调用由 &mut self 串行化。
        unsafe {
            self.device
                .CreateShaderResourceView(&before, None, Some(&mut before_view))
                .map_err(failure)?;
            self.device
                .CreateShaderResourceView(&after, None, Some(&mut after_view))
                .map_err(failure)?;
            self.device
                .CreateUnorderedAccessView(&output, None, Some(&mut output_view))
                .map_err(failure)?;
            self.context.CSSetShader(&self.shader, None);
            self.context
                .CSSetShaderResources(0, Some(&[before_view, after_view]));
            self.context
                .CSSetConstantBuffers(0, Some(&[Some(constants)]));
            self.context
                .CSSetUnorderedAccessViews(0, 1, Some([output_view].as_ptr()), None);
            self.context.Dispatch(columns, rows, 1);
            self.context
                .CSSetUnorderedAccessViews(0, 1, Some([None].as_ptr()), None);
            self.context.CSSetShaderResources(0, Some(&[None, None]));
            self.context.CopyResource(&staging, &output);
        }
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: 离线任务允许等待 GPU；仅映射结果元数据，不在采集线程等待。
        unsafe {
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .map_err(failure)?;
        let values = unsafe {
            std::slice::from_raw_parts(mapped.pData.cast::<u32>(), output_bytes as usize / 4)
        };
        let regions = values
            .chunks_exact(4)
            .filter(|tile| tile[0] < tile[2] && tile[1] < tile[3])
            .map(|tile| argusflow_capture::PixelRect {
                x: tile[0],
                y: tile[1],
                width: tile[2] - tile[0],
                height: tile[3] - tile[1],
            })
            .collect();
        unsafe {
            self.context.Unmap(&staging, 0);
        }
        Ok(argusflow_capture::merge_regions(regions)
            .into_iter()
            .map(|region| InspectionRect {
                x: current.bounds().x + f64::from(region.x),
                y: current.bounds().y + f64::from(region.y),
                width: region.width.into(),
                height: region.height.into(),
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "gpu_difference_tests.rs"]
mod tests;
