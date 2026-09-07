//! 可复用 top-down DIB：BitBlt 后直接冻结原生 BGRX，不再调用 GetDIBits 或逐像素换色。

use argusflow_core::{EvidenceFrame, EvidencePixelFormat, InspectionFailure, InspectionRect};
use std::{ffi::c_void, ptr::NonNull};
use windows::Win32::Graphics::Gdi::*;

/// 仅在创建线程使用的截图缓冲，不实现 Send/Sync；录制摄入线程退出时确定释放。
pub(super) struct EvidenceSurface {
    /// 复用的内存 DC，始终选入本对象拥有的 DIB。
    memory: HDC,
    /// 原生 BGRX top-down 位图，像素内存由 GDI 管理。
    bitmap: HBITMAP,
    /// 销毁 DIB 前必须恢复的 DC 原对象。
    original: HGDIOBJ,
    /// 创建 DIB 返回的内存指针，访问前必须 GdiFlush。
    pixels: NonNull<u8>,
    /// 同尺寸事件复用此缓冲；尺寸变化才重新创建。
    width: u32,
    height: u32,
}

impl EvidenceSurface {
    /// 分配一次可直接读取的位图；参数验证确保切片长度不会溢出。
    pub(super) fn new(width: u32, height: u32) -> Result<Self, InspectionFailure> {
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 32_000_000 {
            return Err(InspectionFailure::InvalidGeometry);
        }
        // SAFETY: DC 与位图全部归当前同步线程所有，失败分支逐一释放已取得资源。
        let memory = unsafe { CreateCompatibleDC(None) };
        if memory.is_invalid() {
            return Err(InspectionFailure::Unavailable);
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap = match unsafe {
            CreateDIBSection(Some(memory), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        } {
            Ok(bitmap) => bitmap,
            Err(_) => {
                unsafe {
                    let _ = DeleteDC(memory);
                }
                return Err(InspectionFailure::Unavailable);
            }
        };
        let Some(pixels) = NonNull::new(bits.cast::<u8>()) else {
            unsafe {
                let _ = DeleteObject(bitmap.into());
                let _ = DeleteDC(memory);
            }
            return Err(InspectionFailure::Unavailable);
        };
        let original = unsafe { SelectObject(memory, bitmap.into()) };
        if original.is_invalid() || original.0 as isize == -1 {
            unsafe {
                let _ = DeleteObject(bitmap.into());
                let _ = DeleteDC(memory);
            }
            return Err(InspectionFailure::Unavailable);
        }
        Ok(Self {
            memory,
            bitmap,
            original,
            pixels,
            width,
            height,
        })
    }

    /// 比较已分配尺寸，不读取或重建任何像素资源。
    pub(super) fn matches(&self, bounds: InspectionRect) -> bool {
        f64::from(self.width) == bounds.width && f64::from(self.height) == bounds.height
    }

    /// 同步完成屏幕拷贝后只做一次 memcpy，返回与可复用 DIB 无关的自有帧。
    pub(super) fn capture(
        &mut self,
        screen: HDC,
        bounds: InspectionRect,
    ) -> Result<EvidenceFrame, InspectionFailure> {
        if !self.matches(bounds) {
            return Err(InspectionFailure::InvalidGeometry);
        }
        // SAFETY: 两个 DC 本线程有效，DIB 独占；GdiFlush 完成绘制后才访问 bits。
        unsafe {
            BitBlt(
                self.memory,
                0,
                0,
                self.width as i32,
                self.height as i32,
                Some(screen),
                bounds.x as i32,
                bounds.y as i32,
                SRCCOPY | CAPTUREBLT,
            )
            .map_err(|_| InspectionFailure::Unavailable)?;
            if !GdiFlush().as_bool() {
                return Err(InspectionFailure::Unavailable);
            }
        }
        self.freeze(bounds)
    }

    /// 已 Flush 且当前线程未提交新的绘制操作，可安全读取完整 top-down 像素。
    fn freeze(&self, bounds: InspectionRect) -> Result<EvidenceFrame, InspectionFailure> {
        // SAFETY: DIB 以 32bpp 创建，每行 width*4 正好 DWORD 对齐；尺寸在构造时已限制。
        let pixels = unsafe {
            std::slice::from_raw_parts(
                self.pixels.as_ptr(),
                self.width as usize * self.height as usize * 4,
            )
        }
        .to_vec();
        EvidenceFrame::new(
            bounds,
            self.width,
            self.height,
            EvidencePixelFormat::Bgrx8,
            pixels,
        )
    }
}

impl Drop for EvidenceSurface {
    fn drop(&mut self) {
        // SAFETY: 在创建线程清空待处理绘制，恢复选入对象后才能删除位图和 DC。
        unsafe {
            let _ = GdiFlush();
            SelectObject(self.memory, self.original);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.memory);
        }
    }
}

/// 短期借用桌面 DC，不缓存可能失效的桌面句柄。
pub(super) struct ScreenDc(pub(super) HDC);
impl ScreenDc {
    pub(super) fn acquire() -> Result<Self, InspectionFailure> {
        // SAFETY: DC 不跨线程传递，Drop 对应 ReleaseDC。
        let dc = unsafe { GetDC(None) };
        if dc.is_invalid() {
            Err(InspectionFailure::Unavailable)
        } else {
            Ok(Self(dc))
        }
    }
}
impl Drop for ScreenDc {
    fn drop(&mut self) {
        unsafe { ReleaseDC(None, self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dib_preserves_top_down_colors_and_owned_frames_survive_reuse() {
        let surface = EvidenceSurface::new(2, 2).unwrap();
        let bounds = InspectionRect {
            x: -10.0,
            y: -20.0,
            width: 2.0,
            height: 2.0,
        };
        // 只绘制测试内存位图，不访问用户屏幕。
        unsafe {
            let _ = SetPixel(
                surface.memory,
                0,
                0,
                windows::Win32::Foundation::COLORREF(0x000000ff),
            );
            let _ = SetPixel(
                surface.memory,
                0,
                1,
                windows::Win32::Foundation::COLORREF(0x00ff0000),
            );
            assert!(GdiFlush().as_bool());
        }
        let frame = surface.freeze(bounds).unwrap();
        let first = frame.clone().into_rgba8();
        assert_eq!(&first.pixels()[..4], &[255, 0, 0, 255]);
        assert_eq!(&first.pixels()[8..12], &[0, 0, 255, 255]);
        unsafe {
            let _ = SetPixel(
                surface.memory,
                0,
                0,
                windows::Win32::Foundation::COLORREF(0x0000ff00),
            );
            assert!(GdiFlush().as_bool());
        }
        let next = surface.freeze(bounds).unwrap().into_rgba8();
        assert_eq!(&next.pixels()[..4], &[0, 255, 0, 255]);
        drop(surface);
        assert_eq!(&frame.into_rgba8().pixels()[..4], &[255, 0, 0, 255]);
        assert!(EvidenceSurface::new(0, 2).is_err());
        assert!(EvidenceSurface::new(u32::MAX, 2).is_err());
    }
}
