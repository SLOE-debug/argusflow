//! 顶层窗口枚举、筛选和只读快照。
use super::identity::WindowIdentity;
use crate::{
    WindowsError as Failure,
    platform::{failure, hwnd},
};
use argusflow_core::FailureKind;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
        },
    },
    core::BOOL,
};

/// 顶层窗口的只读快照。
#[derive(Debug, Clone)]
pub struct WindowInfo {
    identity: WindowIdentity,
    title: String,
    class_name: String,
}
impl WindowInfo {
    /// 窗口身份。
    pub fn identity(&self) -> WindowIdentity {
        self.identity.clone()
    }
    /// 查询时的窗口标题。
    pub fn title(&self) -> &str {
        &self.title
    }
    /// 原生窗口类名。
    pub fn class_name(&self) -> &str {
        &self.class_name
    }
}

/// 顶层窗口精确筛选条件，未指定的字段不参与匹配。
#[derive(Debug, Clone, Default)]
pub struct WindowLocator {
    /// 可选进程 ID。
    pub process_id: Option<u32>,
    /// 精确窗口标题。
    pub title: Option<String>,
    /// 精确原生类名。
    pub class_name: Option<String>,
}

impl WindowLocator {
    /// 枚举可见顶层窗口；仅在匹配后读取进程身份，权限失败明确返回。
    pub fn find_all(&self) -> Result<Vec<WindowInfo>, Failure> {
        let mut handles = Vec::<isize>::with_capacity(4096);
        // SAFETY: callback 同步执行，LPARAM 指向存活的 Vec；callback 不抛出异常。
        unsafe {
            EnumWindows(
                Some(collect_window),
                LPARAM((&mut handles as *mut Vec<isize>) as isize),
            )
        }
        .map_err(|error| {
            if handles.len() == 4096 {
                Failure::new(
                    FailureKind::ResourceLimit,
                    "enum_windows",
                    "顶层窗口数量超过 4096",
                )
            } else {
                failure("enum_windows", error)
            }
        })?;
        let mut result = Vec::new();
        for handle in handles {
            let mut pid = 0;
            // SAFETY: 只读取仍可能被外部销毁的 HWND，后续身份校验处理竞态。
            unsafe { GetWindowThreadProcessId(hwnd(handle), Some(&mut pid)) };
            if self.process_id.is_some_and(|expected| expected != pid) {
                continue;
            }
            let mut title = [0u16; 4096];
            let mut class = [0u16; 256];
            // SAFETY: 两个输出缓冲区均有效，不调用目标应用窗口过程。
            let title_len = unsafe { GetWindowTextW(hwnd(handle), &mut title) } as usize;
            // SAFETY: 同上，GetClassNameW 仅读取窗口类元数据。
            let class_len = unsafe { GetClassNameW(hwnd(handle), &mut class) } as usize;
            let title = String::from_utf16_lossy(&title[..title_len]);
            let class_name = String::from_utf16_lossy(&class[..class_len]);
            if self
                .title
                .as_ref()
                .is_some_and(|expected| expected != &title)
                || self
                    .class_name
                    .as_ref()
                    .is_some_and(|expected| expected != &class_name)
            {
                continue;
            }
            result.push(WindowInfo {
                identity: WindowIdentity::from_handle(handle)?,
                title,
                class_name,
            });
        }
        Ok(result)
    }
    /// 必须恰好匹配一个窗口。
    pub fn find_unique(&self) -> Result<WindowInfo, Failure> {
        let mut windows = self.find_all()?;
        match windows.len() {
            0 => Err(Failure::new(
                FailureKind::NotFound,
                "window_query",
                "没有匹配的窗口",
            )),
            1 => Ok(windows.remove(0)),
            _ => Err(Failure::new(
                FailureKind::Ambiguous,
                "window_query",
                "匹配到多个窗口",
            )),
        }
    }
}

unsafe extern "system" fn collect_window(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: EnumWindows 调用方传入存活 Vec 的指针，callback 在同线程同步调用。
    let handles = unsafe { &mut *(parameter.0 as *mut Vec<isize>) };
    // SAFETY: 枚举提供的窗口句柄仅用于只读查询。
    if unsafe { IsWindowVisible(window) }.as_bool() {
        if handles.len() == 4096 {
            return BOOL(0);
        }
        handles.push(window.0 as isize);
    }
    BOOL(1)
}
