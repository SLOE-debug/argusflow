//! 三槽 GPU 区域读回，提交与 Map 分离；未完成时立即返回调度器。

use argusflow_core::InspectionFailure;
use std::collections::VecDeque;
use windows::Win32::Graphics::{
    Direct3D11::*,
    Dxgi::{
        Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
        DXGI_ERROR_WAS_STILL_DRAWING,
    },
};

/// 紧密排列的原生区域像素。
#[derive(Debug)]
pub(super) struct ReadbackPixels {
    pub(super) region: D3D11_BOX,
    pub(super) pixels: Vec<u8>,
}

#[derive(Debug)]
struct TextureSlot {
    texture: ID3D11Texture2D,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct Slot<T> {
    textures: Vec<TextureSlot>,
    metadata: Option<T>,
    regions: Vec<D3D11_BOX>,
    completed: Vec<ReadbackPixels>,
}

/// 同一设备线程独占使用的固定容量读回队列。
#[derive(Debug)]
pub(super) struct ReadbackQueue<T> {
    slots: [Slot<T>; 3],
    pending: VecDeque<usize>,
}

impl<T> Default for ReadbackQueue<T> {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| Slot {
                textures: Vec::new(),
                metadata: None,
                regions: Vec::new(),
                completed: Vec::new(),
            }),
            pending: VecDeque::new(),
        }
    }
}

impl<T> ReadbackQueue<T> {
    /// 是否仍有可以提交的空闲槽。
    pub(super) fn available(&self) -> bool {
        self.pending.len() < 3
    }
    pub(super) fn pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// 仅提交区域复制；metadata 保持与这个 GPU 版本绑定。
    pub(super) fn submit(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        source: &ID3D11Texture2D,
        regions: Vec<D3D11_BOX>,
        metadata: T,
    ) -> Result<(), InspectionFailure> {
        let index = self
            .slots
            .iter()
            .position(|slot| slot.metadata.is_none())
            .ok_or(InspectionFailure::Unavailable)?;
        let slot = &mut self.slots[index];
        for (index, region) in regions.iter().enumerate() {
            if region.right <= region.left || region.bottom <= region.top {
                return Err(InspectionFailure::InvalidGeometry);
            }
            let width = region.right - region.left;
            let height = region.bottom - region.top;
            if u64::from(width) * u64::from(height) > 32_000_000 {
                return Err(InspectionFailure::InvalidGeometry);
            }
            if slot
                .textures
                .get(index)
                .is_none_or(|texture| texture.width != width || texture.height != height)
            {
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
                let mut texture = None;
                // SAFETY: 正尺寸布局已经校验，设备与 source 属于同一适配器线程。
                unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }
                    .map_err(|_| InspectionFailure::Unavailable)?;
                let texture = TextureSlot {
                    texture: texture.ok_or(InspectionFailure::Unavailable)?,
                    width,
                    height,
                };
                if index == slot.textures.len() {
                    slot.textures.push(texture);
                } else {
                    slot.textures[index] = texture;
                }
            }
            unsafe {
                context.CopySubresourceRegion(
                    &slot.textures[index].texture,
                    0,
                    0,
                    0,
                    0,
                    source,
                    0,
                    Some(region),
                );
            }
        }
        slot.textures.truncate(regions.len());
        slot.regions = regions;
        slot.completed.clear();
        slot.metadata = Some(metadata);
        self.pending.push_back(index);
        unsafe {
            context.Flush();
        }
        Ok(())
    }

    /// 按提交顺序尝试交付一帧；每个资源每次调用最多尝试一次 Map。
    pub(super) fn poll(
        &mut self,
        context: &ID3D11DeviceContext,
    ) -> Result<Option<(T, Vec<ReadbackPixels>)>, InspectionFailure> {
        let Some(index) = self.pending.front().copied() else {
            return Ok(None);
        };
        let slot = &mut self.slots[index];
        while slot.completed.len() < slot.regions.len() {
            let index = slot.completed.len();
            let texture = &slot.textures[index];
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            // SAFETY: 每个槽在完成前不会被复用，纹理以 CPU_READ 创建。
            match unsafe {
                context.Map(
                    &texture.texture,
                    0,
                    D3D11_MAP_READ,
                    D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                    Some(&mut mapped),
                )
            } {
                Err(error) if error.code() == DXGI_ERROR_WAS_STILL_DRAWING => return Ok(None),
                Err(_) => return Err(InspectionFailure::Unavailable),
                Ok(()) => {}
            }
            let row_bytes = texture.width as usize * 4;
            let result = if mapped.pData.is_null() || (mapped.RowPitch as usize) < row_bytes {
                Err(InspectionFailure::InvalidGeometry)
            } else {
                let length = (mapped.RowPitch as usize)
                    .checked_mul(texture.height as usize)
                    .ok_or(InspectionFailure::InvalidGeometry);
                length.map(|length| {
                    // SAFETY: Map 成功，RowPitch 与高度限定借用范围，下面必定 Unmap。
                    let source =
                        unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>(), length) };
                    let mut pixels = Vec::with_capacity(row_bytes * texture.height as usize);
                    for row in 0..texture.height as usize {
                        let start = row * mapped.RowPitch as usize;
                        pixels.extend_from_slice(&source[start..start + row_bytes]);
                    }
                    ReadbackPixels {
                        region: slot.regions[index],
                        pixels,
                    }
                })
            };
            unsafe {
                context.Unmap(&texture.texture, 0);
            }
            slot.completed.push(result?);
        }
        self.pending.pop_front();
        let metadata = slot.metadata.take().ok_or(InspectionFailure::Unavailable)?;
        Ok(Some((metadata, std::mem::take(&mut slot.completed))))
    }
}
