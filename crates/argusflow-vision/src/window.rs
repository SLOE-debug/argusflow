//! 进程级窗口发现契约；平台枚举与视觉运行时通过此边界解耦。

use argusflow_core::WindowIdentity;
use serde::{Deserialize, Serialize};

use crate::{PhysicalRect, VisionError};

/// 一个可独立捕获的顶层窗口实例。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowDescriptor {
    /// HWND/PID 组合身份。
    pub identity: WindowIdentity,
    /// Owner HWND；没有 Owner 的普通顶层窗口为空。
    pub owner_handle: Option<u64>,
    /// 当前桌面 Z-Order，数值越小越靠前。
    pub z_order: usize,
    /// DWM 可见边界，使用虚拟屏幕物理坐标。
    pub screen_bounds: PhysicalRect,
    /// 是否为当前前台窗口。
    pub foreground: bool,
}

/// 枚举指定进程当前所有可捕获顶层窗口的平台契约。
pub trait WindowInventory: std::fmt::Debug + Send + Sync {
    /// 返回按 Z-Order 排序、已经过滤不可见和 Cloaked 项的窗口。
    fn windows_for_process(&self, process_id: u32) -> Result<Vec<WindowDescriptor>, VisionError>;
}
