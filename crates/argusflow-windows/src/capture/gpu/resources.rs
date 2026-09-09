//! 资源先计费再创建，最后一个纹理引用消失时归还额度。
use super::{Graphics, failure, invalid};
use argusflow_capture_contracts::{CaptureResult, Reservation};
use windows::Win32::Graphics::{
    Direct3D11::*,
    Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
};
pub(in crate::capture) struct Texture {
    pub native: ID3D11Texture2D,
    pub width: u32,
    pub height: u32,
    _bytes: Reservation,
}
impl Texture {
    pub fn new(graphics: &Graphics, width: u32, height: u32, staging: bool) -> CaptureResult<Self> {
        if width == 0 || height == 0 || width > 16384 || height > 16384 {
            return Err(invalid("texture dimensions"));
        }
        // staging 是 GPU 资源对象，GPU 预算也计费，CPU 映射输出另按实际缓冲计费。
        let bytes = graphics
            .budget
            .reserve(width as usize * height as usize * 4)?;
        let descriptor = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: if staging {
                D3D11_USAGE_STAGING
            } else {
                D3D11_USAGE_DEFAULT
            },
            BindFlags: if staging {
                0
            } else {
                D3D11_BIND_SHADER_RESOURCE.0 as u32
            },
            CPUAccessFlags: if staging {
                D3D11_CPU_ACCESS_READ.0 as u32
            } else {
                0
            },
            ..Default::default()
        };
        let mut native = None;
        // SAFETY: 尺寸已经验证，资源描述与绑定标志一致。
        unsafe {
            graphics
                .device
                .CreateTexture2D(&descriptor, None, Some(&mut native))
        }
        .map_err(failure)?;
        Ok(Self {
            native: native.ok_or_else(|| invalid("missing texture"))?,
            width,
            height,
            _bytes: bytes,
        })
    }
    pub fn view(&self, graphics: &Graphics) -> CaptureResult<ID3D11ShaderResourceView> {
        let mut view = None;
        // SAFETY: 仅对本模块创建的 GPU shader resource 纹理调用。
        unsafe {
            graphics
                .device
                .CreateShaderResourceView(&self.native, None, Some(&mut view))
        }
        .map_err(failure)?;
        view.ok_or_else(|| invalid("missing texture view"))
    }
}
pub(in crate::capture) struct Buffer {
    pub native: ID3D11Buffer,
    _bytes: Reservation,
}
impl Buffer {
    pub fn new(
        graphics: &Graphics,
        bytes: u32,
        stride: u32,
        bind: D3D11_BIND_FLAG,
        staging: bool,
        initial: Option<&[u8]>,
    ) -> CaptureResult<Self> {
        if bytes == 0 || initial.is_some_and(|data| data.len() != bytes as usize) {
            return Err(invalid("buffer size"));
        }
        let reservation = graphics.budget.reserve(bytes as usize)?;
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: bytes,
            Usage: if staging {
                D3D11_USAGE_STAGING
            } else {
                D3D11_USAGE_DEFAULT
            },
            BindFlags: bind.0 as u32,
            CPUAccessFlags: if staging {
                D3D11_CPU_ACCESS_READ.0 as u32
            } else {
                0
            },
            MiscFlags: if stride > 0 && !staging {
                D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32
            } else {
                0
            },
            StructureByteStride: stride,
        };
        let data = initial.map(|bytes| D3D11_SUBRESOURCE_DATA {
            pSysMem: bytes.as_ptr().cast(),
            ..Default::default()
        });
        let mut native = None;
        // SAFETY: 初始化缓冲长度等于 ByteWidth，D3D11 返回前复制数据。
        unsafe {
            graphics.device.CreateBuffer(
                &desc,
                data.as_ref().map(|data| data as *const _),
                Some(&mut native),
            )
        }
        .map_err(failure)?;
        Ok(Self {
            native: native.ok_or_else(|| invalid("missing buffer"))?,
            _bytes: reservation,
        })
    }
}
