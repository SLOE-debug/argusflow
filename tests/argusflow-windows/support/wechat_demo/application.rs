//! 按可执行文件身份附加或启动应用；不拥有也不终止用户应用。
use argusflow_core::{Operation, OperationOptions};
use argusflow_windows::WindowIdentity;
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_NO_MORE_FILES, HANDLE, HWND, LPARAM},
        System::{
            Diagnostics::ToolHelp::*,
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
        UI::WindowsAndMessaging::*,
    },
    core::BOOL,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// 应用身份通过路径匹配，标题仅用于同一进程内消歧。
pub struct ApplicationTarget {
    /// 真实 EXE 的绝对路径。
    pub executable: PathBuf,
    /// 精确匹配主窗口标题。
    pub title: String,
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: 此守卫唯一持有成功获得的进程或快照句柄。
        let _ = unsafe { CloseHandle(self.0) };
    }
}
/// 按进程身份定位或启动应用，不把已找到窗口等同于已取得前台。
pub async fn attach(target: &ApplicationTarget) -> Result<WindowIdentity> {
    if !target.executable.is_absolute() || !target.executable.is_file() {
        return Err("应用目标必须是现存的绝对 EXE 路径".into());
    }
    let executable = target.executable.canonicalize()?;
    let mut pids = processes(&executable)?;
    if pids.is_empty() {
        // 不使用 shell 或 kill-on-close Job；demo 结束后应用继续运行。
        let _child = std::process::Command::new(&executable).spawn()?;
        println!("已启动目标应用进程");
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let window = loop {
        let candidates = windows_for(&pids, &target.title)?;
        match candidates.as_slice() {
            [window] => break WindowIdentity::from_handle(*window)?,
            [] => {}
            _ => return Err("目标进程存在多个同标题窗口，请明确唯一主窗口".into()),
        }
        if Instant::now() >= deadline {
            return Err("目标进程未出现指定主窗口".into());
        }
        tokio::time::sleep(Duration::from_millis(80)).await;
        pids = processes(&executable)?;
    };
    Ok(window)
}
/// 定位进程窗口，恢复隐藏/最小化状态，然后请求正常前台激活。
pub async fn activate(target: &ApplicationTarget) -> Result<WindowIdentity> {
    let window = attach(target).await?;
    let hwnd = HWND(window.handle() as *mut _);
    // SAFETY: 身份已经验证，只读取所选窗口状态。
    let command = if unsafe { IsIconic(hwnd) }.as_bool() {
        SW_RESTORE
    } else {
        SW_SHOW
    };
    // SAFETY: 异步恢复/显示所选窗口，不等待外部窗口过程。
    let _ = unsafe { ShowWindowAsync(hwnd, command) };
    tokio::time::sleep(Duration::from_millis(120)).await;
    let operation = Operation::new(OperationOptions::default());
    let activation = window.activate(&operation);
    // SetForegroundWindow 可能已接受请求，但跨线程激活尚未完成；只等状态，不重发。
    while window.require_foreground().is_err() && operation.remaining() > Duration::from_millis(20)
    {
        window.validate()?;
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    if window.require_foreground().is_err() {
        activation?;
        return Err("Windows 未将目标应用置于前台".into());
    }
    println!("按进程路径激活窗口，PID={}", window.process_id());
    Ok(window)
}
fn processes(executable: &Path) -> Result<Vec<u32>> {
    let filename = executable
        .file_name()
        .ok_or("EXE 文件名缺失")?
        .to_string_lossy();
    // SAFETY: 只枚举系统进程，不获得写入权限。
    let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }?);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    // SAFETY: entry 是大小正确的独占输出，快照句柄有效。
    unsafe { Process32FirstW(snapshot.0, &mut entry) }?;
    let mut pids = Vec::new();
    loop {
        let end = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szExeFile.len());
        if String::from_utf16_lossy(&entry.szExeFile[..end]).eq_ignore_ascii_case(&filename) {
            // SAFETY: 只对同名候选进程读取元数据，守卫关闭句柄。
            let process = Handle(unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    entry.th32ProcessID,
                )
            }?);
            let mut path = vec![0u16; 32768];
            let mut length = path.len() as u32;
            // SAFETY: 输出长度不超过 UTF-16 缓冲大小。
            unsafe {
                QueryFullProcessImageNameW(
                    process.0,
                    PROCESS_NAME_WIN32,
                    windows::core::PWSTR(path.as_mut_ptr()),
                    &mut length,
                )
            }?;
            let actual =
                PathBuf::from(String::from_utf16_lossy(&path[..length as usize])).canonicalize()?;
            if actual
                .to_string_lossy()
                .eq_ignore_ascii_case(&executable.to_string_lossy())
            {
                pids.push(entry.th32ProcessID);
            }
        }
        // SAFETY: 同上，枚举结束仅接受 ERROR_NO_MORE_FILES。
        if let Err(error) = unsafe { Process32NextW(snapshot.0, &mut entry) } {
            if error.code() == windows::core::HRESULT::from_win32(ERROR_NO_MORE_FILES.0) {
                break;
            }
            return Err(error.into());
        }
    }
    Ok(pids)
}
struct Selection<'a> {
    pids: &'a [u32],
    title: &'a str,
    handles: Vec<isize>,
}
fn windows_for(pids: &[u32], title: &str) -> Result<Vec<isize>> {
    let mut selection = Selection {
        pids,
        title,
        handles: Vec::new(),
    };
    // SAFETY: callback 同步执行，参数指向仍存活的独占状态。
    unsafe {
        EnumWindows(
            Some(collect),
            LPARAM((&mut selection as *mut Selection<'_>) as isize),
        )
    }?;
    Ok(selection.handles)
}
unsafe extern "system" fn collect(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: windows_for 在同步 EnumWindows 期间持有 Selection。
    let selection = unsafe { &mut *(parameter.0 as *mut Selection<'_>) };
    let mut pid = 0;
    // SAFETY: 仅读取元数据；窗口可能消失，WindowIdentity 会重新验证。
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    if selection.pids.contains(&pid) {
        let mut title = [0u16; 4096];
        // SAFETY: title 为有效独占输出。
        let length = unsafe { GetWindowTextW(window, &mut title) } as usize;
        if String::from_utf16_lossy(&title[..length]) == selection.title {
            selection.handles.push(window.0 as isize);
        }
    }
    BOOL(1)
}
