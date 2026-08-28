//! 目标窗口及其同进程 owned popup 的轻量拓扑快照。

use std::ffi::c_void;

use argusflow_core::WindowIdentity;
use argusflow_vision::{PhysicalRect, TopologyGeneration, VisionError};
use serde::{Deserialize, Serialize};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    UI::WindowsAndMessaging::{
        EnumWindows, GW_OWNER, GetWindow, GetWindowRect, GetWindowThreadProcessId, IsWindow,
        IsWindowVisible,
    },
};
use windows::core::BOOL;

use super::error::capture_error;

/// 同一窗口作用域内的拓扑角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowRole {
    /// AppSession 获取的主窗口。
    Primary,
    /// 由主窗口拥有的弹窗、菜单或临时对话框。
    OwnedPopup,
    /// 同一进程的其它可见顶层窗口。
    SameProcessTopLevel,
}

/// 当前 Windows capture 实际交付给 VisionRuntime 的 surface 范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSurfaceMode {
    /// 只捕获 AppSession 的 primary HWND；其它条目只用于拓扑失效检测。
    PrimaryOnly,
}

/// 拓扑中的一个窗口条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowTopologyEntry {
    /// 可复验的 HWND/PID 身份。
    pub identity: WindowIdentity,
    /// 当前条目的拓扑角色。
    pub role: WindowRole,
    /// 屏幕物理像素矩形；读取失败时为空。
    pub bounds: Option<PhysicalRect>,
}

/// 一次捕获时使用的窗口拓扑快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowTopology {
    /// 主窗口身份。
    pub primary: WindowIdentity,
    /// 当前捕获实现真实承诺的 surface 范围。
    pub surface_mode: CaptureSurfaceMode,
    /// 当前拓扑代数。
    pub generation: TopologyGeneration,
    /// 同进程可见顶层窗口及 owned popup；这些条目不会自动加入 capture surface set。
    pub windows: Vec<WindowTopologyEntry>,
}

/// 通过枚举同进程顶层窗口追踪拓扑变化。
#[derive(Debug, Default)]
pub struct WindowTopologyTracker {
    /// 最近一次观察到的窗口身份集合。
    last_identities: Option<Vec<WindowIdentity>>,
    /// 身份集合变化次数。
    generation: u64,
}

impl WindowTopologyTracker {
    /// 创建空拓扑追踪器。
    pub const fn new() -> Self {
        Self {
            last_identities: None,
            generation: 1,
        }
    }

    /// 刷新指定 AppSession 窗口的拓扑并返回当前代数。
    pub fn refresh(&mut self, primary: WindowIdentity) -> Result<WindowTopology, VisionError> {
        let primary_hwnd = native_window(primary.handle);
        if !unsafe { IsWindow(Some(primary_hwnd)) }.as_bool() {
            return Err(VisionError::CaptureUnavailable {
                message: "primary window no longer exists".to_owned(),
            });
        }
        let mut process_id = 0_u32;
        // SAFETY: process_id 是同步 Win32 调用的独占输出，HWND 已通过 IsWindow 校验。
        unsafe { GetWindowThreadProcessId(primary_hwnd, Some(&mut process_id)) };
        if process_id != primary.process_id {
            return Err(VisionError::WindowIdentityChanged {
                expected: primary,
                actual: Some(WindowIdentity {
                    handle: primary.handle,
                    process_id,
                }),
            });
        }

        let mut search = TopologySearch {
            primary,
            primary_hwnd,
            entries: vec![WindowTopologyEntry {
                identity: primary,
                role: WindowRole::Primary,
                bounds: window_bounds(primary_hwnd),
            }],
        };
        let parameter = LPARAM((&mut search as *mut TopologySearch) as isize);
        // SAFETY: EnumWindows 同步执行 callback；parameter 在调用返回前始终有效且独占。
        unsafe { EnumWindows(Some(enum_window), parameter) }
            .map_err(|error| capture_error("failed to enumerate visual window topology", error))?;

        search.entries.sort_by_key(|entry| entry.identity.handle);
        let identities = search
            .entries
            .iter()
            .map(|entry| entry.identity)
            .collect::<Vec<_>>();
        if self.last_identities.as_ref() != Some(&identities) {
            if self.last_identities.is_some() {
                self.generation = self.generation.saturating_add(1);
            }
            self.last_identities = Some(identities);
        }
        Ok(WindowTopology {
            primary,
            surface_mode: CaptureSurfaceMode::PrimaryOnly,
            generation: TopologyGeneration::new(self.generation),
            windows: search.entries,
        })
    }
}

/// EnumWindows callback 只收集属于目标进程的可见顶层窗口。
unsafe extern "system" fn enum_window(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: parameter 来自 refresh 的同步栈帧，EnumWindows 返回前不会失效。
    let search = unsafe { &mut *(parameter.0 as *mut TopologySearch) };
    if window == search.primary_hwnd || !unsafe { IsWindowVisible(window) }.as_bool() {
        return true.into();
    }
    let mut process_id = 0_u32;
    // SAFETY: process_id 是同步调用期间的独占输出，window 由系统枚举提供。
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    if process_id != search.primary.process_id {
        return true.into();
    }
    let identity = WindowIdentity {
        handle: window.0 as usize as u64,
        process_id,
    };
    let role = unsafe { GetWindow(window, GW_OWNER) }
        .ok()
        .filter(|owner| owner == &search.primary_hwnd)
        .map(|_| WindowRole::OwnedPopup)
        .unwrap_or(WindowRole::SameProcessTopLevel);
    search.entries.push(WindowTopologyEntry {
        identity,
        role,
        bounds: window_bounds(window),
    });
    true.into()
}

/// 读取屏幕物理像素窗口矩形；窗口在枚举过程中消失时保留身份但不猜测矩形。
fn window_bounds(window: HWND) -> Option<PhysicalRect> {
    let mut rect = RECT::default();
    // SAFETY: rect 是同步 Win32 调用的独占输出，window 由系统或已校验身份提供。
    unsafe { GetWindowRect(window, &mut rect) }
        .ok()
        .and_then(|_| {
            let width = i64::from(rect.right) - i64::from(rect.left);
            let height = i64::from(rect.bottom) - i64::from(rect.top);
            (width > 0 && height > 0)
                .then(|| PhysicalRect::new(rect.left, rect.top, width as u32, height as u32))?
        })
}

/// 把领域层的不透明 HWND 表示恢复为 Windows 类型。
fn native_window(handle: u64) -> HWND {
    HWND(handle as usize as *mut c_void)
}

/// EnumWindows 回调使用的同步结果。
struct TopologySearch {
    /// 主窗口的稳定身份。
    primary: WindowIdentity,
    /// 主窗口的原生句柄。
    primary_hwnd: HWND,
    /// 已收集的拓扑条目。
    entries: Vec<WindowTopologyEntry>,
}
