//! 可见目标窗口的 GDI 局部采集；不读取整桌面，不改变窗口捕获保护设置。
use super::desktop::Desktop;
use argusflow_capture_contracts::ScreenRect;
use std::error::Error;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::*,
    UI::{
        HiDpi::*,
        WindowsAndMessaging::{GetWindowDisplayAffinity, WDA_NONE},
    },
};

/// 只在同步采集期间持有 GDI 资源，所有返回路径恢复 DPI 与原位图。
struct Resources {
    window: HWND,
    source: HDC,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    dpi: DPI_AWARENESS_CONTEXT,
}
impl Drop for Resources {
    fn drop(&mut self) {
        // SAFETY: 所有句柄只在本次同步调用创建；先恢复选择再删除位图和 DC。
        unsafe {
            if !self.previous.0.is_null() {
                SelectObject(self.memory, self.previous);
            }
            if !self.bitmap.0.is_null() {
                let _ = DeleteObject(self.bitmap.into());
            }
            if !self.memory.0.is_null() {
                let _ = DeleteDC(self.memory);
            }
            if !self.source.0.is_null() {
                ReleaseDC(Some(self.window), self.source);
            }
            if !self.dpi.0.is_null() {
                SetThreadDpiAwarenessContext(self.dpi);
            }
        }
    }
}
/// 返回 top-down BGRX；窗口必须前台可见，区域不得超出窗口。
pub async fn capture(
    desktop: &Desktop,
    bounds: ScreenRect,
    region: ScreenRect,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let owned = Desktop {
        input: desktop.input.clone(),
        window: desktop.window.clone(),
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    // PrintWindow 是同步外部调用；超时后结束 demo，不重试或创建替代采集线程。
    std::thread::Builder::new()
        .name("wechat-window-capture".into())
        .spawn(move || {
            let _ = sender
                .send(capture_sync(&owned, bounds, region).map_err(|error| error.to_string()));
        })?;
    tokio::time::timeout(std::time::Duration::from_secs(3), receiver)
        .await
        .map_err(|_| "窗口采集超时；本次 demo 停止")?
        .map_err(|_| "窗口采集线程异常退出")?
        .map_err(Into::into)
}

fn capture_sync(
    desktop: &Desktop,
    bounds: ScreenRect,
    region: ScreenRect,
) -> Result<Vec<u8>, Box<dyn Error>> {
    if desktop.bounds()? != bounds {
        return Err("窗口在截图前已移动".into());
    }
    let x = region.x() - bounds.x();
    let y = region.y() - bounds.y();
    if x < 0
        || y < 0
        || i64::from(x) + i64::from(region.width()) > i64::from(bounds.width())
        || i64::from(y) + i64::from(region.height()) > i64::from(bounds.height())
    {
        return Err("截图区域超出窗口".into());
    }
    let window = HWND(desktop.window.handle() as *mut _);
    let mut affinity = 0;
    // SAFETY: 只读取已验证目标窗口的捕获保护；拒绝受保护的目标。
    unsafe { GetWindowDisplayAffinity(window, &mut affinity) }?;
    if affinity != WDA_NONE.0 {
        return Err("目标窗口禁止捕获".into());
    }
    let mut resources = Resources {
        window,
        source: HDC::default(),
        memory: HDC::default(),
        bitmap: HBITMAP::default(),
        previous: HGDIOBJ::default(),
        dpi: DPI_AWARENESS_CONTEXT::default(),
    };
    // SAFETY: 本采集线程独占 DPI 上下文，守卫在所有路径恢复。
    resources.dpi =
        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    if resources.dpi.0.is_null() {
        return Err("设置截图 DPI 失败".into());
    }
    // SAFETY: 目标窗口已经验证，返回的 DC 由 resources 释放。
    resources.source = unsafe { GetWindowDC(Some(window)) };
    if resources.source.0.is_null() {
        return Err("GetWindowDC 失败".into());
    }
    // SAFETY: source 是有效 DC，新建内存 DC 独占属于当前线程。
    resources.memory = unsafe { CreateCompatibleDC(Some(resources.source)) };
    if resources.memory.0.is_null() {
        return Err("CreateCompatibleDC 失败".into());
    }
    let width: i32 = bounds.width().try_into()?;
    let height: i32 = bounds.height().try_into()?;
    if u64::from(bounds.width()) * u64::from(bounds.height()) > 16_000_000 {
        return Err("截图超过像素预算".into());
    }
    // SAFETY: DC 有效且尺寸经过像素预算与整数范围验证。
    resources.bitmap = unsafe { CreateCompatibleBitmap(resources.source, width, height) };
    if resources.bitmap.0.is_null() {
        return Err("CreateCompatibleBitmap 失败".into());
    }
    // SAFETY: 此位图不属于任何其他 DC，守卫会恢复此前选择。
    resources.previous = unsafe { SelectObject(resources.memory, resources.bitmap.into()) };
    if resources.previous.0.is_null() || resources.previous.0 as isize == -1 {
        resources.previous = HGDIOBJ::default();
        return Err("SelectObject 失败".into());
    }
    // PrintWindow 可能重置 DC 原点，因此先绘制窗口，再在像素缓冲里明确裁剪。
    // SAFETY: HWND 已验证，DC 及已选位图在同步调用期间存活。
    unsafe {
        windows::Win32::Storage::Xps::PrintWindow(
            window,
            resources.memory,
            windows::Win32::Storage::Xps::PRINT_WINDOW_FLAGS(2),
        )
    }
    .ok()?;
    // SAFETY: 恢复同一 DC 的原对象，GetDIBits 要求位图未被选入 DC。
    unsafe { SelectObject(resources.memory, resources.previous) };
    resources.previous = HGDIOBJ::default();
    let mut bytes = vec![0u8; bounds.width() as usize * bounds.height() as usize * 4];
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: 位图已经取消选择，输出缓冲覆盖完整 32 位像素；info 独占。
    if unsafe {
        GetDIBits(
            resources.memory,
            resources.bitmap,
            0,
            bounds.height(),
            Some(bytes.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        )
    } != height
    {
        return Err("GetDIBits 未返回完整截图".into());
    }
    if desktop.bounds()? != bounds {
        return Err("窗口在截图期间移动".into());
    }
    let mut cropped = Vec::with_capacity(region.width() as usize * region.height() as usize * 4);
    for row in y as usize..y as usize + region.height() as usize {
        let start = (row * bounds.width() as usize + x as usize) * 4;
        cropped.extend_from_slice(&bytes[start..start + region.width() as usize * 4]);
    }
    Ok(cropped)
}
