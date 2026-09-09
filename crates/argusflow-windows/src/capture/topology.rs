//! 显示输出发现与物理像素坐标转换。
use super::gpu::{failure, invalid};
use argusflow_capture_contracts::*;
use windows::{
    Win32::Graphics::Dxgi::{Common::*, *},
    core::Interface,
};

pub(super) struct OutputInfo {
    pub output: IDXGIOutput1,
    pub info: SourceInfo,
    pub raw_width: u32,
    pub raw_height: u32,
}
pub(super) fn outputs(
    adapter: &IDXGIAdapter1,
    adapter_index: u32,
    generation: u64,
) -> CaptureResult<Vec<OutputInfo>> {
    let mut found = Vec::new();
    let mut index = 0;
    loop {
        // SAFETY: DXGI 枚举只读取适配器的当前输出。
        let output = match unsafe { adapter.EnumOutputs(index) } {
            Ok(output) => output,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => return Err(failure(error)),
        };
        index += 1;
        if index > 32 {
            return Err(invalid("too many display outputs"));
        }
        let desc = unsafe { output.GetDesc() }.map_err(failure)?;
        if !desc.AttachedToDesktop.as_bool() {
            continue;
        }
        let output: IDXGIOutput1 = output.cast().map_err(failure)?;
        let rotation = match desc.Rotation {
            DXGI_MODE_ROTATION_IDENTITY | DXGI_MODE_ROTATION_UNSPECIFIED => Rotation::Identity,
            DXGI_MODE_ROTATION_ROTATE90 => Rotation::Clockwise90,
            DXGI_MODE_ROTATION_ROTATE180 => Rotation::Clockwise180,
            DXGI_MODE_ROTATION_ROTATE270 => Rotation::Clockwise270,
            _ => return Err(invalid("unknown output rotation")),
        };
        let bounds = ScreenRect::new(
            desc.DesktopCoordinates.left,
            desc.DesktopCoordinates.top,
            (i64::from(desc.DesktopCoordinates.right) - i64::from(desc.DesktopCoordinates.left))
                as u32,
            (i64::from(desc.DesktopCoordinates.bottom) - i64::from(desc.DesktopCoordinates.top))
                as u32,
        )?;
        let size = match rotation {
            Rotation::Clockwise90 | Rotation::Clockwise270 => (bounds.height(), bounds.width()),
            _ => (bounds.width(), bounds.height()),
        };
        let name = String::from_utf16_lossy(
            &desc.DeviceName[..desc
                .DeviceName
                .iter()
                .position(|x| *x == 0)
                .unwrap_or(desc.DeviceName.len())],
        );
        // SourceId 根据设备名稳定分配，输出枚举顺序只用于适配器内部定位。
        let mut hash = 14695981039346656037_u64;
        for byte in name.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(1099511628211);
        }
        let id = SourceId(hash ^ ((u64::from(adapter_index) + 1) << 48));
        let mut state = SourceState::Initializing;
        let mut diagnostic = None;
        let output6: IDXGIOutput6 = output.cast().map_err(failure)?;
        // SAFETY: Windows 10/11 输出具备 DXGI 1.6 描述，用于显式拒绝 HDR。
        let extended = unsafe { output6.GetDesc1() }.map_err(failure)?;
        if extended.ColorSpace != DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709 {
            state = SourceState::Unavailable;
            diagnostic = Some(CaptureError::new(
                argusflow_core::FailureKind::Unsupported,
                "dxgi_color_space",
                "当前输出不是受支持的 SDR 色彩空间",
            ));
        }
        let mut dpi_x = 0;
        let mut dpi_y = 0;
        // SAFETY: HMONITOR 来自当前输出，调用线程设置为 Per Monitor V2。
        unsafe {
            windows::Win32::UI::HiDpi::GetDpiForMonitor(
                desc.Monitor,
                windows::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI,
                &mut dpi_x,
                &mut dpi_y,
            )
        }
        .map_err(failure)?;
        found.push(OutputInfo {
            output,
            info: SourceInfo {
                id,
                name,
                generation,
                bounds,
                rotation,
                dpi: (dpi_x, dpi_y),
                state,
                failure: diagnostic,
            },
            raw_width: size.0,
            raw_height: size.1,
        });
    }
    Ok(found)
}

/// 从未旋转的纹理区域转为屏幕方向。
pub(super) fn to_logical(
    rect: PixelRect,
    width: u32,
    height: u32,
    rotation: Rotation,
) -> CaptureResult<PixelRect> {
    match rotation {
        Rotation::Identity => Ok(rect),
        Rotation::Clockwise90 => PixelRect::new(
            height - rect.bottom(),
            rect.x(),
            rect.height(),
            rect.width(),
        ),
        Rotation::Clockwise180 => PixelRect::new(
            width - rect.right(),
            height - rect.bottom(),
            rect.width(),
            rect.height(),
        ),
        Rotation::Clockwise270 => {
            PixelRect::new(rect.y(), width - rect.right(), rect.height(), rect.width())
        }
    }
}
pub(super) fn to_raw(
    rect: PixelRect,
    width: u32,
    height: u32,
    rotation: Rotation,
) -> CaptureResult<PixelRect> {
    let inverse = match rotation {
        Rotation::Identity => Rotation::Identity,
        Rotation::Clockwise90 => Rotation::Clockwise270,
        Rotation::Clockwise180 => Rotation::Clockwise180,
        Rotation::Clockwise270 => Rotation::Clockwise90,
    };
    let (w, h) = match rotation {
        Rotation::Clockwise90 | Rotation::Clockwise270 => (height, width),
        _ => (width, height),
    };
    if !PixelRect::new(0, 0, w, h)?.contains(rect) {
        return Err(invalid("logical region outside source"));
    }
    to_logical(rect, w, h, inverse)
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/capture/topology.rs"]
mod tests;
