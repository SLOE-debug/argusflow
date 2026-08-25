//! 根据显式 EXE 契约复用、启动并恢复 direct-process Windows 桌面应用。

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use argusflow_core::{ApplicationTarget, AutomationError, BackendKind, WindowTitleMatcher};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM},
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
            IsWindowVisible, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindowAsync,
        },
    },
    core::{BOOL, PWSTR},
};

/// UIA executor 可安全跨线程传递的稳定窗口身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedWindow {
    /// 原生 HWND 的无符号表示。
    pub handle: u64,
    /// HWND 当前所属的进程 ID。
    pub process_id: u32,
}

/// 通过显式应用契约管理 Windows 顶层窗口的无状态服务。
#[derive(Debug, Default)]
pub struct WindowService;

impl WindowService {
    /// 在阻塞线程中复用或启动 direct-process 应用，并返回已经恢复的唯一窗口。
    pub async fn resolve_application(
        &self,
        target: ApplicationTarget,
    ) -> Result<ResolvedWindow, AutomationError> {
        tokio::task::spawn_blocking(move || resolve_application_blocking(&target))
            .await
            .map_err(|error| backend_failed(format!("application resolver task failed: {error}")))?
    }
}

/// 完整应用解析只运行在 Tokio blocking pool，避免启动等待阻塞异步调度线程。
fn resolve_application_blocking(
    target: &ApplicationTarget,
) -> Result<ResolvedWindow, AutomationError> {
    let executable_path = validate_executable_path(target)?;
    let existing = find_windows(target, &executable_path, None)?;
    if let Some(window) = require_unique_window(target, existing)? {
        restore_window(window)?;
        request_foreground(window);
        return Ok(window);
    }

    let mut child = Command::new(&executable_path)
        .args(&target.arguments)
        .spawn()
        .map_err(|error| {
            backend_failed(format!(
                "failed to launch '{}': {error}",
                executable_path.display()
            ))
        })?;
    let process_id = child.id();
    let deadline = Instant::now() + Duration::from_millis(target.launch_timeout_ms);
    loop {
        let candidates = find_windows(target, &executable_path, Some(process_id))?;
        if let Some(window) = require_unique_window(target, candidates)? {
            restore_window(window)?;
            request_foreground(window);
            return Ok(window);
        }
        if let Some(status) = child.try_wait().map_err(|error| {
            backend_failed(format!(
                "failed to observe launched process {process_id}: {error}"
            ))
        })? {
            return Err(backend_failed(format!(
                "launched process {process_id} exited with {status} before creating a matching window"
            )));
        }
        if Instant::now() >= deadline {
            return Err(backend_failed(format!(
                "application did not create a matching window within {} ms: {}",
                target.launch_timeout_ms,
                describe_target(target)
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// 验证执行契约使用绝对且存在的普通文件路径，避免 shell 和 PATH 猜测。
fn validate_executable_path(target: &ApplicationTarget) -> Result<PathBuf, AutomationError> {
    let configured = Path::new(target.executable_path.trim());
    if !configured.is_absolute() {
        return Err(backend_failed(
            "application executable path must be absolute",
        ));
    }
    if !configured.is_file() {
        return Err(backend_failed(format!(
            "application executable does not exist: {}",
            configured.display()
        )));
    }
    configured.canonicalize().map_err(|error| {
        backend_failed(format!(
            "failed to canonicalize application executable '{}': {error}",
            configured.display()
        ))
    })
}

/// 枚举同一 EXE 或指定新进程创建的可见顶层窗口。
fn find_windows(
    target: &ApplicationTarget,
    executable_path: &Path,
    required_process_id: Option<u32>,
) -> Result<Vec<ResolvedWindow>, AutomationError> {
    let mut search = WindowSearch {
        target,
        executable_path,
        required_process_id,
        matches: Vec::new(),
    };
    let parameter = LPARAM((&mut search as *mut WindowSearch<'_>) as isize);
    // SAFETY: EnumWindows 同步调用 callback；parameter 在调用期间指向有效且独占的栈状态。
    unsafe { EnumWindows(Some(enum_window), parameter) }
        .map_err(|error| backend_failed(format!("failed to enumerate desktop windows: {error}")))?;
    Ok(search.matches)
}

/// 同步窗口枚举所需的只读筛选条件与结果缓冲区。
struct WindowSearch<'a> {
    /// 应用标题和启动约束。
    target: &'a ApplicationTarget,
    /// 已规范化的目标 EXE 路径。
    executable_path: &'a Path,
    /// 启动后只接受本次子进程创建的窗口。
    required_process_id: Option<u32>,
    /// 按系统枚举顺序收集的匹配窗口。
    matches: Vec<ResolvedWindow>,
}

/// `EnumWindows` 回调只读取窗口元数据并把稳定 HWND/PID 写入调用方状态。
unsafe extern "system" fn enum_window(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: parameter 由 find_windows 传入，并在整个同步 EnumWindows 调用期间保持有效。
    let search = unsafe { &mut *(parameter.0 as *mut WindowSearch<'_>) };
    // SAFETY: window 由 EnumWindows 提供，只执行只读状态查询。
    if !unsafe { IsWindowVisible(window) }.as_bool() {
        return true.into();
    }
    let mut process_id = 0_u32;
    // SAFETY: process_id 在同步调用期间有效且独占，window 由系统枚举提供。
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    if process_id == 0
        || search
            .required_process_id
            .is_some_and(|required| required != process_id)
    {
        return true.into();
    }
    if search.required_process_id.is_none()
        && !process_path_matches(process_id, search.executable_path)
    {
        return true.into();
    }
    let title = window_title(window);
    if !title_matches(&search.target.window_title, &title) {
        return true.into();
    }
    search.matches.push(ResolvedWindow {
        handle: window.0 as usize as u64,
        process_id,
    });
    true.into()
}

/// 读取进程完整映像路径并按 Windows 路径大小写规则比较。
fn process_path_matches(process_id: u32, expected: &Path) -> bool {
    process_executable_path(process_id)
        .and_then(|path| path.canonicalize().ok())
        .is_some_and(|actual| path_eq_ignore_case(&actual, expected))
}

/// 通过受限查询句柄读取进程映像路径；系统进程或已退出进程返回 None。
fn process_executable_path(process_id: u32) -> Option<PathBuf> {
    // SAFETY: 只请求映像路径查询权限，不继承句柄；process_id 来自窗口所属进程。
    let handle =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }.ok()?;
    let handle = ProcessHandle(handle);
    let mut buffer = vec![0_u16; 32_768];
    let mut length = u32::try_from(buffer.len()).ok()?;
    // SAFETY: buffer 可写且长度由 length 精确描述，handle 在调用期间保持有效。
    unsafe {
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    }
    .ok()?;
    let length = usize::try_from(length).ok()?;
    Some(PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
}

/// 自动关闭 OpenProcess 返回的内核句柄。
struct ProcessHandle(HANDLE);

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: 句柄由 OpenProcess 成功创建，且此 RAII 包装拥有唯一关闭责任。
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// 读取可见顶层窗口标题；读取失败按空标题处理并自然不匹配非空规则。
fn window_title(window: HWND) -> String {
    // SAFETY: window 由 EnumWindows 提供，只读取标题长度。
    let length = unsafe { GetWindowTextLengthW(window) };
    if length <= 0 {
        return String::new();
    }
    let mut buffer = vec![0_u16; length as usize + 1];
    // SAFETY: buffer 可写且包含结尾空间，window 在当前枚举回调期间有效。
    let copied = unsafe { GetWindowTextW(window, &mut buffer) };
    String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
}

/// 使用 Unicode 小写形式实现 UI 配置要求的大小写不敏感标题匹配。
fn title_matches(matcher: &WindowTitleMatcher, title: &str) -> bool {
    let title = title.to_lowercase();
    match matcher {
        WindowTitleMatcher::Equal { value } => title == value.to_lowercase(),
        WindowTitleMatcher::Contains { value } => title.contains(&value.to_lowercase()),
    }
}

/// 强制应用作用域解析为唯一窗口，防止隐式选择多个实例。
fn require_unique_window(
    target: &ApplicationTarget,
    windows: Vec<ResolvedWindow>,
) -> Result<Option<ResolvedWindow>, AutomationError> {
    match windows.as_slice() {
        [] => Ok(None),
        [window] => Ok(Some(*window)),
        _ => Err(AutomationError::AmbiguousTarget {
            query: describe_target(target),
            matches: windows.len(),
        }),
    }
}

/// 恢复最小化窗口并把恢复结果作为 UIA 查询的硬条件。
fn restore_window(window: ResolvedWindow) -> Result<(), AutomationError> {
    let native = HWND(window.handle as usize as *mut std::ffi::c_void);
    // SAFETY: HWND 来自刚完成的 EnumWindows，ShowWindowAsync 不解引用调用方内存。
    unsafe {
        let _ = ShowWindowAsync(
            native,
            if IsIconic(native).as_bool() {
                SW_RESTORE
            } else {
                SW_SHOW
            },
        );
    }
    for _ in 0..20 {
        // SAFETY: 调用只读取刚枚举窗口的最小化状态。
        if !unsafe { IsIconic(native) }.as_bool() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(backend_failed(format!(
        "Windows did not restore application window HWND={}",
        window.handle
    )))
}

/// 请求把已恢复窗口带到前台；Windows foreground-lock 拒绝不会阻断语义 UIA。
fn request_foreground(window: ResolvedWindow) {
    let native = HWND(window.handle as usize as *mut std::ffi::c_void);
    // SAFETY: HWND 来自刚完成的窗口枚举；调用不解引用外部内存，失败由系统返回 FALSE。
    let _ = unsafe { SetForegroundWindow(native) };
}

/// Windows 路径比较遵循文件系统常见的大小写不敏感规则。
fn path_eq_ignore_case(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

/// 生成用于错误日志和歧义报告的稳定应用描述。
fn describe_target(target: &ApplicationTarget) -> String {
    format!(
        "application(executable_path = {:?}, window_title = {:?})",
        target.executable_path, target.window_title
    )
}

/// 把应用生命周期失败归入已选择 UIA 后端的不可回退执行错误。
fn backend_failed(message: impl Into<String>) -> AutomationError {
    AutomationError::BackendFailed {
        backend: BackendKind::WindowsUia,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use argusflow_core::WindowTitleMatcher;

    use super::{path_eq_ignore_case, title_matches};

    #[test]
    fn window_title_matching_is_case_insensitive() {
        assert!(title_matches(
            &WindowTitleMatcher::Contains {
                value: "NOTEPAD++".to_owned(),
            },
            "new 1 - Notepad++",
        ));
        assert!(title_matches(
            &WindowTitleMatcher::Equal {
                value: "ArgusFlow".to_owned(),
            },
            "argusflow",
        ));
    }

    #[test]
    fn windows_paths_are_compared_without_ascii_case() {
        assert!(path_eq_ignore_case(
            Path::new(r"C:\Program Files\Notepad++\notepad++.exe"),
            Path::new(r"c:\program files\notepad++\NOTEPAD++.EXE"),
        ));
    }
}
