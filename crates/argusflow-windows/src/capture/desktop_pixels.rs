//! 桌面物理坐标到 DXGI 原始纹理的裁切、旋转与紧密行复制。

use argusflow_core::{InspectionFailure, InspectionRect};
use windows::Win32::{
    Foundation::RECT,
    Graphics::Dxgi::Common::{
        DXGI_MODE_ROTATION, DXGI_MODE_ROTATION_IDENTITY, DXGI_MODE_ROTATION_ROTATE90,
        DXGI_MODE_ROTATION_ROTATE180, DXGI_MODE_ROTATION_ROTATE270,
    },
};

/// 已验证的显示器方向；未定义的驱动值直接拒绝。
#[derive(Clone, Copy)]
pub(super) enum Rotation {
    Identity,
    Clockwise90,
    Clockwise180,
    Clockwise270,
}

impl TryFrom<DXGI_MODE_ROTATION> for Rotation {
    type Error = InspectionFailure;
    fn try_from(value: DXGI_MODE_ROTATION) -> Result<Self, Self::Error> {
        match value {
            DXGI_MODE_ROTATION_IDENTITY => Ok(Self::Identity),
            DXGI_MODE_ROTATION_ROTATE90 => Ok(Self::Clockwise90),
            DXGI_MODE_ROTATION_ROTATE180 => Ok(Self::Clockwise180),
            DXGI_MODE_ROTATION_ROTATE270 => Ok(Self::Clockwise270),
            _ => Err(InspectionFailure::InvalidGeometry),
        }
    }
}

/// 一个显示器与目标截图的非空交集；坐标均为物理像素。
#[derive(Clone, Copy)]
pub(super) struct OutputCrop {
    /// 原始纹理到桌面坐标的旋转。
    rotation: Rotation,
    /// 完整输出在旋转后的桌面尺寸。
    output_width: usize,
    output_height: usize,
    /// 交集在该显示器中的起点。
    source_x: usize,
    source_y: usize,
    /// 交集在最终截图中的起点。
    target_x: usize,
    target_y: usize,
    /// 交集尺寸。
    width: usize,
    height: usize,
    /// 最终截图的紧密行宽度和总高度。
    target_width: usize,
    target_height: usize,
}

impl OutputCrop {
    /// GPU 已裁切后的局部纹理使用同一旋转算法，目标位置保持原屏幕拼接位置。
    pub(super) fn copy_cropped(
        &self,
        source: &[u8],
        stride: usize,
        width: u32,
        height: u32,
        target: &mut [u8],
    ) -> Result<(), InspectionFailure> {
        let local = Self {
            source_x: 0,
            source_y: 0,
            output_width: self.width,
            output_height: self.height,
            ..*self
        };
        local.copy(source, stride, width, height, target)
    }
    /// 将桌面裁切范围逆旋转到原始纹理，GPU 只读回这块区域。
    pub(super) fn texture_region(&self) -> windows::Win32::Graphics::Direct3D11::D3D11_BOX {
        let (left, top, width, height) = match self.rotation {
            Rotation::Identity => (self.source_x, self.source_y, self.width, self.height),
            Rotation::Clockwise90 => (
                self.source_y,
                self.output_width - self.source_x - self.width,
                self.height,
                self.width,
            ),
            Rotation::Clockwise180 => (
                self.output_width - self.source_x - self.width,
                self.output_height - self.source_y - self.height,
                self.width,
                self.height,
            ),
            Rotation::Clockwise270 => (
                self.output_height - self.source_y - self.height,
                self.source_x,
                self.height,
                self.width,
            ),
        };
        windows::Win32::Graphics::Direct3D11::D3D11_BOX {
            left: left as u32,
            top: top as u32,
            front: 0,
            right: (left + width) as u32,
            bottom: (top + height) as u32,
            back: 1,
        }
    }

    /// 分离几何计算与逐行复制；负屏幕坐标不会被直接转成无符号数。
    pub(super) fn new(bounds: InspectionRect, output: RECT, rotation: Rotation) -> Option<Self> {
        let left = bounds.x.max(f64::from(output.left));
        let top = bounds.y.max(f64::from(output.top));
        let right = (bounds.x + bounds.width).min(f64::from(output.right));
        let bottom = (bounds.y + bounds.height).min(f64::from(output.bottom));
        if right <= left || bottom <= top {
            return None;
        }
        Some(Self {
            rotation,
            output_width: (i64::from(output.right) - i64::from(output.left)) as usize,
            output_height: (i64::from(output.bottom) - i64::from(output.top)) as usize,
            source_x: (left - f64::from(output.left)) as usize,
            source_y: (top - f64::from(output.top)) as usize,
            target_x: (left - bounds.x) as usize,
            target_y: (top - bounds.y) as usize,
            width: (right - left) as usize,
            height: (bottom - top) as usize,
            target_width: bounds.width as usize,
            target_height: bounds.height as usize,
        })
    }

    /// 将已映射的 BGRA 行复制进最终 BGRX 帧；横屏使用整行 memcpy。
    pub(super) fn copy(
        &self,
        source: &[u8],
        stride: usize,
        texture_width: u32,
        texture_height: u32,
        target: &mut [u8],
    ) -> Result<(), InspectionFailure> {
        let width = texture_width as usize;
        let height = texture_height as usize;
        let physical_size = match self.rotation {
            Rotation::Identity | Rotation::Clockwise180 => (width, height),
            Rotation::Clockwise90 | Rotation::Clockwise270 => (height, width),
        };
        if physical_size != (self.output_width, self.output_height)
            || stride < width * 4
            || stride
                .checked_mul(height)
                .is_none_or(|length| source.len() < length)
            || self
                .target_width
                .checked_mul(self.target_height)
                .and_then(|n| n.checked_mul(4))
                != Some(target.len())
        {
            return Err(InspectionFailure::InvalidGeometry);
        }
        for row in 0..self.height {
            let destination = ((self.target_y + row) * self.target_width + self.target_x) * 4;
            let y = self.source_y + row;
            if matches!(self.rotation, Rotation::Identity) {
                let origin = y * stride + self.source_x * 4;
                target[destination..destination + self.width * 4]
                    .copy_from_slice(&source[origin..origin + self.width * 4]);
                continue;
            }
            for column in 0..self.width {
                let x = self.source_x + column;
                // DXGI 返回未旋转纹理，按显示方向逆变换才能恢复用户实际看到的像素。
                let (source_x, source_y) = match self.rotation {
                    Rotation::Identity => (x, y),
                    Rotation::Clockwise90 => (y, height - 1 - x),
                    Rotation::Clockwise180 => (width - 1 - x, height - 1 - y),
                    Rotation::Clockwise270 => (width - 1 - y, x),
                };
                let origin = source_y * stride + source_x * 4;
                let destination = destination + column * 4;
                target[destination..destination + 4].copy_from_slice(&source[origin..origin + 4]);
            }
        }
        Ok(())
    }
}
