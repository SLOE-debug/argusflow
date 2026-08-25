//! ApplicationSpec 对应的 Win32 顶层窗口和进程身份发现。

use std::path::{Path, PathBuf};

use argusflow_core::{ApplicationError, ApplicationSpec, WindowIdentity, WindowTitleMatcher};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM},
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
            IsWindowVisible,
        },
    },
    core::{BOOL, PWSTR},
};

/// 验证执行契约使用绝对且存在的普通文件路径，避免 shell 和 PATH 猜测。
pub(super) fn validate_executable_path(
    spec: &ApplicationSpec,
) -> Result<PathBuf, ApplicationError> {
    let configured = Path::new(spec.executable_path.trim());
    if !configured.is_absolute() {
        return Err(ApplicationError::InvalidSpec {
            message: "application executable path must be absolute".to_owned(),
        });
    }
    if !configured.is_file() {
        return Err(ApplicationError::InvalidSpec {
            message: format!(
                "application executable does not exist: {}",
                configured.display()
            ),
        });
    }
    configured
        .canonicalize()
        .map_err(|error| ApplicationError::InvalidSpec {
            message: format!(
                "failed to canonicalize application executable '{}': {error}",
                configured.display()
            ),
        })
}

/// 枚举同一 EXE 或指定新进程创建的可见顶层窗口。
pub(super) fn find_windows(
    spec: &ApplicationSpec,
    executable_path: &Path,
    required_process_id: Option<u32>,
) -> Result<Vec<WindowIdentity>, ApplicationError> {
    let mut search = WindowSearch {
        spec,
        executable_path,
        required_process_id,
        matches: Vec::new(),
    };
    let parameter = LPARAM((&mut search as *mut WindowSearch<'_>) as isize);
    // SAFETY: EnumWindows 同步调用 callback；parameter 在调用期间指向有效且独占的栈状态。
    unsafe { EnumWindows(Some(enum_window), parameter) }.map_err(|error| {
        ApplicationError::LaunchFailed {
            message: format!("failed to enumerate desktop windows: {error}"),
        }
    })?;
    Ok(search.matches)
}

/// 同步窗口枚举所需的只读筛选条件与结果缓冲区。
struct WindowSearch<'a> {
    /// 应用标题和启动约束。
    spec: &'a ApplicationSpec,
    /// 已规范化的目标 EXE 路径。
    executable_path: &'a Path,
    /// 启动后只接受本次子进程创建的窗口。
    required_process_id: Option<u32>,
    /// 按系统枚举顺序收集的匹配窗口。
    matches: Vec<WindowIdentity>,
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
    if !title_matches(&search.spec.window_title, &window_title(window)) {
        return true.into();
    }
    search.matches.push(WindowIdentity {
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

/// 读取可见顶层窗口标题；读取失败按空标题处理。
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

/// 使用 Unicode 小写形式实现配置要求的大小写不敏感标题匹配。
fn title_matches(matcher: &WindowTitleMatcher, title: &str) -> bool {
    let title = title.to_lowercase();
    match matcher {
        WindowTitleMatcher::Equal { value } => title == value.to_lowercase(),
        WindowTitleMatcher::Contains { value } => title.contains(&value.to_lowercase()),
    }
}

/// Windows 路径比较遵循文件系统常见的大小写不敏感规则。
fn path_eq_ignore_case(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
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
    }

    #[test]
    fn windows_paths_are_compared_without_ascii_case() {
        assert!(path_eq_ignore_case(
            Path::new(r"C:\Program Files\Notepad++\notepad++.exe"),
            Path::new(r"c:\program files\notepad++\NOTEPAD++.EXE"),
        ));
    }
}
