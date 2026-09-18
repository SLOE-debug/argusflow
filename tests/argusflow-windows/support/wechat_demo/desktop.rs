//! Windows 适配：窗口物理区域、鼠标与 Unicode 输入；进程激活独立处理。
use argusflow_capture_contracts::ScreenRect;
use argusflow_core::{ClickCount, Key, MouseButton, OperationOptions, ScreenPoint};
use argusflow_windows::{InputAction, InputService, WindowIdentity};
use std::error::Error;
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::{
        HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
        WindowsAndMessaging::GetWindowRect,
    },
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// 仅持有目标窗口和可关闭的输入服务，不包含微信业务判断。
pub struct Desktop {
    /// 复用已有 SendInput 服务；结束 demo 时显式关闭。
    pub input: InputService,
    /// 当前目标窗口的身份租约。
    pub window: WindowIdentity,
}
impl Desktop {
    /// 读取前台窗口的物理像素边界；允许多显示器负坐标。
    pub fn bounds(&self) -> Result<ScreenRect> {
        self.window.require_foreground()?;
        let mut rect = RECT::default();
        // SAFETY: 同步读取期间切换当前线程 DPI 上下文，返回前立即恢复；不跨 await。
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if previous.0.is_null() {
            return Err("无法建立物理像素 DPI 上下文".into());
        }
        // SAFETY: 窗口身份已验证，rect 是独占有效输出。
        let result = unsafe { GetWindowRect(HWND(self.window.handle() as *mut _), &mut rect) };
        // SAFETY: 恢复本线程原来的有效上下文。
        unsafe { SetThreadDpiAwarenessContext(previous) };
        result?;
        Ok(ScreenRect::new(
            rect.left,
            rect.top,
            (rect.right - rect.left).try_into()?,
            (rect.bottom - rect.top).try_into()?,
        )?)
    }
    /// 复验窗口位置后单击，底层拒绝遮挡与错误前台。
    pub async fn click(&self, point: ScreenPoint, observed_bounds: ScreenRect) -> Result<()> {
        if self.bounds()? != observed_bounds {
            return Err("窗口在观察后移动，拒绝旧坐标".into());
        }
        self.input
            .perform(
                self.window.clone(),
                InputAction::Click {
                    point,
                    button: MouseButton::Left,
                    count: ClickCount::Single,
                },
                OperationOptions::default(),
            )
            .await?;
        Ok(())
    }
    /// 向已建立焦点输入 Unicode 文本，不使用剪贴板。
    pub async fn type_text(&self, text: &str) -> Result<()> {
        self.input
            .perform(
                self.window.clone(),
                InputAction::Text(text.into()),
                OperationOptions::default(),
            )
            .await?;
        Ok(())
    }
    /// 发送一次按键，不自动重试副作用。
    pub async fn key(&self, key: Key) -> Result<()> {
        self.input
            .perform(
                self.window.clone(),
                InputAction::Chord(vec![key]),
                OperationOptions::default(),
            )
            .await?;
        Ok(())
    }
}
