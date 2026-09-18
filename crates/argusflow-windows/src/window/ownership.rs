//! 输入和视觉采样共享窗口归属规则；同进程不代表同一个交互范围。
use crate::{
    WindowIdentity, WindowsError,
    platform::{failure, hwnd},
};
use argusflow_core::{FailureKind, ScreenPoint};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    UI::WindowsAndMessaging::*,
};

impl WindowIdentity {
    /// 确认屏幕命中对象属于目标窗口、其 owned 窗口或正在展开的菜单。
    pub fn validate_input_point(&self, point: ScreenPoint) -> Result<(), WindowsError> {
        self.validate()?;
        let _dpi = crate::platform::PhysicalDpi::enter()?;
        // SAFETY: 只读命中测试；不根据旧 OCR/UIA 边界放行被遮挡的位置。
        let hit = unsafe {
            WindowFromPoint(POINT {
                x: point.x,
                y: point.y,
            })
        };
        if !self.owns_surface(hit)? {
            return Err(WindowsError::new(
                FailureKind::InvalidInput,
                "input",
                "目标位置被其他窗口遮挡或不属于目标窗口",
            ));
        }
        let mut rect = RECT::default();
        // SAFETY: 命中 HWND 只用于同步查询；失效会返回错误。
        unsafe { GetWindowRect(hit, &mut rect) }.map_err(|e| failure("input_bounds", e))?;
        if point.x < rect.left
            || point.x >= rect.right
            || point.y < rect.top
            || point.y >= rect.bottom
        {
            return Err(WindowsError::new(
                FailureKind::StaleHandle,
                "input",
                "命中窗口已移动，输入位置失效",
            ));
        }
        Ok(())
    }

    pub(super) fn owns_surface(&self, candidate: HWND) -> Result<bool, WindowsError> {
        if owner_chain(candidate, hwnd(self.handle()), self.process_id()) {
            return Ok(true);
        }
        // 系统菜单通常没有 GW_OWNER；必须同时验证菜单类、所属线程及该线程当前的菜单 owner。
        let mut pid = 0;
        // SAFETY: 仅查询窗口的线程、进程和系统注册类。
        let thread = unsafe { GetWindowThreadProcessId(candidate, Some(&mut pid)) };
        if thread == 0 || pid != self.process_id() {
            return Ok(false);
        }
        let mut class = [0u16; 32];
        let length = unsafe { GetClassNameW(candidate, &mut class) } as usize;
        if class[..length] != [35, 51, 50, 55, 54, 56] {
            return Ok(false);
        } // #32768
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        // SAFETY: info 大小已初始化；明确查询菜单所属线程，不使用模糊的前台线程。
        unsafe { GetGUIThreadInfo(thread, &mut info) }
            .map_err(|e| failure("window_menu_owner", e))?;
        Ok((info.flags & GUI_INMENUMODE).0 != 0
            && owner_chain(info.hwndMenuOwner, hwnd(self.handle()), self.process_id())
            && unsafe { GetWindowThreadProcessId(info.hwndMenuOwner, None) } == thread)
    }
}

fn owner_chain(mut candidate: HWND, target: HWND, pid: u32) -> bool {
    // 限制外部窗口图的遍历，防止销毁/重建期间反常的 owner 链使调用无界。
    for _ in 0..64 {
        if candidate.0.is_null() {
            return false;
        }
        let mut actual_pid = 0;
        // SAFETY: 都是只读查询；失效句柄的线程/PID 为零，按不属于处理。
        unsafe { GetWindowThreadProcessId(candidate, Some(&mut actual_pid)) };
        if actual_pid != pid {
            return false;
        }
        if candidate == target {
            return true;
        }
        let root = unsafe { GetAncestor(candidate, GA_ROOT) };
        if root == target {
            return true;
        }
        candidate = unsafe { GetWindow(root, GW_OWNER) }.unwrap_or_default();
    }
    false
}
#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/window/ownership.rs"]
mod tests;
