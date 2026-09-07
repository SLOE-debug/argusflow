//! 基准自有动态窗口；不操作用户应用，退出自动销毁。

use argusflow_core::{EvidenceFrame, WindowIdentity};
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, RECT},
        Graphics::{Dwm::DwmFlush, Gdi::*},
        System::Threading::GetCurrentProcessId,
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
    core::w,
};

/// 显式 --fixture 模式创建无边框窗口，绘制和呈现均在截图计时外。
pub(super) struct CaptureFixture {
    window: HWND,
    width: i32,
    height: i32,
    /// 在窗口创建和读取期间维持物理像素坐标。
    dpi: DPI_AWARENESS_CONTEXT,
}
impl CaptureFixture {
    /// 仅允许主屏幕内的正尺寸测试窗口。
    pub(super) fn new(width: i32, height: i32) -> windows::core::Result<Self> {
        let dpi =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if width <= 0
            || height <= 0
            || width > unsafe { GetSystemMetrics(SM_CXSCREEN) }
            || height > unsafe { GetSystemMetrics(SM_CYSCREEN) }
        {
            unsafe { SetThreadDpiAwarenessContext(dpi) };
            return Err(windows::core::Error::from_hresult(
                windows::Win32::Foundation::E_INVALIDARG,
            ));
        }
        // SAFETY: 系统自带 STATIC 类，无自定义回调，不夺取键盘焦点。
        let result = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("ArgusFlow capture benchmark"),
                WS_POPUP | WS_VISIBLE,
                0,
                0,
                width,
                height,
                None,
                None,
                None,
                None,
            )
        };
        match result {
            Ok(window) => {
                std::thread::sleep(std::time::Duration::from_millis(250));
                Ok(Self {
                    window,
                    width,
                    height,
                    dpi,
                })
            }
            Err(error) => {
                unsafe { SetThreadDpiAwarenessContext(dpi) };
                Err(error)
            }
        }
    }
    /// 只查询本基准进程的窗口。
    pub(super) fn identity(&self) -> WindowIdentity {
        WindowIdentity {
            handle: self.window.0 as u64,
            process_id: unsafe { GetCurrentProcessId() },
        }
    }
    /// 逐轮改变四个象限，检测旧帧、颜色通道和方向错误。
    pub(super) fn paint(&self, iteration: u32) -> windows::core::Result<()> {
        // SAFETY: 排空本窗口的绘制消息，防止默认背景覆盖基准图案。
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, Some(self.window), 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        let dc = unsafe { GetDC(Some(self.window)) };
        if dc.is_invalid() {
            return Err(windows::core::Error::from_thread());
        }
        for quadrant in 0..4 {
            let rect = RECT {
                left: if quadrant % 2 == 0 { 0 } else { self.width / 2 },
                top: if quadrant < 2 { 0 } else { self.height / 2 },
                right: if quadrant % 2 == 0 {
                    self.width / 2
                } else {
                    self.width
                },
                bottom: if quadrant < 2 {
                    self.height / 2
                } else {
                    self.height
                },
            };
            let [red, green, blue] = color(iteration, quadrant);
            let brush = unsafe {
                CreateSolidBrush(COLORREF(
                    u32::from(red) | u32::from(green) << 8 | u32::from(blue) << 16,
                ))
            };
            let success = unsafe { FillRect(dc, &rect, brush) } != 0;
            unsafe {
                let _ = DeleteObject(brush.into());
            }
            if !success {
                unsafe { ReleaseDC(Some(self.window), dc) };
                return Err(windows::core::Error::from_thread());
            }
        }
        unsafe {
            let _ = GdiFlush();
            ReleaseDC(Some(self.window), dc);
            DwmFlush()?;
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
        Ok(())
    }
    /// 校验已知四个像素，完整帧仍由真实截图 API 获取，绝不保存到磁盘。
    pub(super) fn verify(&self, frame: &EvidenceFrame, iteration: u32) -> bool {
        frame.width() == self.width as u32
            && frame.height() == self.height as u32
            && (0..4).all(|quadrant| {
                let x = if quadrant % 2 == 0 {
                    self.width / 4
                } else {
                    self.width * 3 / 4
                };
                let y = if quadrant < 2 {
                    self.height / 4
                } else {
                    self.height * 3 / 4
                };
                let index = (y as usize * frame.width() as usize + x as usize) * 4;
                let [red, green, blue] = color(iteration, quadrant);
                let valid = frame.pixels()[index..index + 3] == [blue, green, red];
                if !valid {
                    eprintln!(
                        "quadrant={quadrant} actual={:?} expected={:?}",
                        &frame.pixels()[index..index + 3],
                        [blue, green, red]
                    );
                }
                valid
            })
    }
}
/// 每轮与象限独立变色，不能只校验一张恒定图像。
fn color(iteration: u32, quadrant: u32) -> [u8; 3] {
    [
        ((iteration * 19 + quadrant * 43) % 200 + 20) as u8,
        ((iteration * 37 + quadrant * 61) % 200 + 20) as u8,
        ((iteration * 53 + quadrant * 29) % 200 + 20) as u8,
    ]
}
impl Drop for CaptureFixture {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
            SetThreadDpiAwarenessContext(self.dpi);
        }
    }
}
