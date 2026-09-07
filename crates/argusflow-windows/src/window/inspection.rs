//! 录制 worker 的只读窗口定位；点击只按屏幕点定位，绝不使用前台窗口替代。

use argusflow_core::{
    InspectionContext, InspectionFailure, InspectionProbe, InspectionRect, WindowIdentity,
    WindowInspector,
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT},
        Graphics::Gdi::ClientToScreen,
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
        UI::{
            HiDpi::{
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
                SetThreadDpiAwarenessContext,
            },
            WindowsAndMessaging::{
                EnumChildWindows, GA_ROOT, GetAncestor, GetClassNameW, GetClientRect,
                GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
                IsWindowVisible, WindowFromPoint,
            },
        },
    },
    core::{BOOL, PWSTR},
};

/// 无状态窗口反查器；不激活或修改用户窗口。
pub struct WindowsWindowInspector;

impl WindowInspector for WindowsWindowInspector {
    fn window_context(
        &self,
        window: WindowIdentity,
    ) -> Result<InspectionContext, InspectionFailure> {
        // SAFETY: 同步线程局部 DPI 范围；身份在读取后再次核对。
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let result = inspect_window(HWND(window.handle as *mut _)).and_then(|context| {
            if context.window == window {
                Ok(context)
            } else {
                Err(InspectionFailure::ContextChanged)
            }
        });
        if !previous.0.is_null() {
            unsafe { SetThreadDpiAwarenessContext(previous) };
        }
        result
    }
    fn context(&self, probe: InspectionProbe) -> Result<InspectionContext, InspectionFailure> {
        // SAFETY: DPI context 仅覆盖当前同步 worker 调用，并在返回前恢复。
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let result = inspect_context(probe);
        // SAFETY: previous 是上一个线程 DPI context；无效值不用于恢复。
        if !previous.0.is_null() {
            unsafe { SetThreadDpiAwarenessContext(previous) };
        }
        result
    }
}

/// 所有原生查询留在本线程；只把自有字符串、数值与矩形返回给异步层。
fn inspect_context(probe: InspectionProbe) -> Result<InspectionContext, InspectionFailure> {
    // SAFETY: Win32 返回借用 HWND，Point 保持 Hook 的物理坐标空间。
    let child = unsafe {
        match probe {
            InspectionProbe::Point(point) => WindowFromPoint(POINT {
                x: point.x,
                y: point.y,
            }),
            InspectionProbe::Focus => GetForegroundWindow(),
            InspectionProbe::Window => return Err(InspectionFailure::ContextChanged),
        }
    };
    // SAFETY: 不解引用 HWND，失效句柄由后续 API 检测。
    let root = unsafe { GetAncestor(child, GA_ROOT) };
    inspect_window(root)
}

/// 从事件指定的根窗口读取自有元数据。
fn inspect_window(root: HWND) -> Result<InspectionContext, InspectionFailure> {
    if root.0.is_null() {
        return Err(InspectionFailure::ContextChanged);
    }
    let mut process_id = 0;
    // SAFETY: PID 输出指针指向当前栈，调用同步完成。
    unsafe { GetWindowThreadProcessId(root, Some(&mut process_id)) };
    if process_id == 0 {
        return Err(InspectionFailure::ContextChanged);
    }
    let bounds = window_bounds(root)?;
    let mut search = RendererSearch {
        viewport: None,
        count: 0,
    };
    // SAFETY: EnumChildWindows 同步完成；callback 仅借用此栈状态。
    unsafe {
        let _ = EnumChildWindows(
            Some(root),
            Some(renderer_child),
            LPARAM((&mut search as *mut RendererSearch) as isize),
        );
    }
    let mut title = [0_u16; 1024];
    let mut class_name = [0_u16; 256];
    // SAFETY: 缓冲区长度由 windows-rs 传递；均为只读窗口查询。
    let title_len = unsafe { GetWindowTextW(root, &mut title) } as usize;
    let class_len = unsafe { GetClassNameW(root, &mut class_name) } as usize;
    let dpi = unsafe { GetDpiForWindow(root) };
    Ok(InspectionContext {
        window: WindowIdentity {
            handle: root.0 as u64,
            process_id,
        },
        executable_path: process_path(process_id),
        title: String::from_utf16_lossy(&title[..title_len]),
        class_name: String::from_utf16_lossy(&class_name[..class_len]),
        bounds,
        browser_viewport: if search.count == 1 {
            search.viewport
        } else {
            None
        },
        dpi,
        has_keyboard_focus: unsafe { GetForegroundWindow() } == root,
    })
}

/// Renderer client 才是网页区域；多个可见 renderer（例如 docked DevTools）拒绝猜测。
struct RendererSearch {
    /// 当前唯一可见 renderer 的物理矩形。
    viewport: Option<InspectionRect>,
    /// 可见 renderer 总数。
    count: usize,
}

unsafe extern "system" fn renderer_child(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: parameter 来自 inspect_context 的同步枚举栈。
    let search = unsafe { &mut *(parameter.0 as *mut RendererSearch) };
    let mut class = [0_u16; 128];
    // SAFETY: HWND 来自枚举且缓冲区有效。
    let length = unsafe { GetClassNameW(window, &mut class) } as usize;
    if String::from_utf16_lossy(&class[..length]) == "Chrome_RenderWidgetHostHWND"
        && unsafe { IsWindowVisible(window) }.as_bool()
    {
        let mut rect = RECT::default();
        let mut origin = POINT::default();
        // SAFETY: 输出参数均是独占栈对象，线程处于 physical DPI context。
        if unsafe { GetClientRect(window, &mut rect) }.is_ok()
            && unsafe { ClientToScreen(window, &mut origin) }.as_bool()
        {
            let bounds = InspectionRect {
                x: f64::from(origin.x),
                y: f64::from(origin.y),
                width: f64::from(rect.right - rect.left),
                height: f64::from(rect.bottom - rect.top),
            };
            if bounds.is_valid() {
                search.count += 1;
                search.viewport = Some(bounds);
            }
        }
    }
    true.into()
}

/// 读取顶层窗口的 physical bounds，拒绝已销毁窗口。
fn window_bounds(window: HWND) -> Result<InspectionRect, InspectionFailure> {
    let mut rect = RECT::default();
    // SAFETY: RECT 输出地址有效；GetWindowRect 验证借用 HWND。
    unsafe { GetWindowRect(window, &mut rect) }.map_err(|_| InspectionFailure::ContextChanged)?;
    let bounds = InspectionRect {
        x: f64::from(rect.left),
        y: f64::from(rect.top),
        width: f64::from(rect.right - rect.left),
        height: f64::from(rect.bottom - rect.top),
    };
    if bounds.is_valid() {
        Ok(bounds)
    } else {
        Err(InspectionFailure::InvalidGeometry)
    }
}

/// 权限不足时保留 None，不伪造进程路径。
fn process_path(process_id: u32) -> Option<String> {
    // SAFETY: 只申请查询权限；成功 handle 在本函数同步关闭。
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }.ok()?;
    let mut path = vec![0_u16; 32768];
    let mut length = path.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        )
    };
    // SAFETY: process 是此函数唯一拥有的有效句柄。
    let _ = unsafe { CloseHandle(process) };
    result
        .ok()
        .map(|()| String::from_utf16_lossy(&path[..length as usize]))
}
