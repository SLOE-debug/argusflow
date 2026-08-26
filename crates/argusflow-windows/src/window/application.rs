//! 根据显式 EXE 契约获取、恢复和清理 direct-process Windows 桌面应用会话。

use std::{
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

use argusflow_core::{
    AcquirePolicy, ActivationPolicy, AppSession, ApplicationError, ApplicationSessionProvider,
    ApplicationSpec, CapabilityId, CapabilitySet, CleanupPolicy, ProcessIdentity, ResourceId,
    WindowIdentity,
};
use async_trait::async_trait;
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{
        GetWindowThreadProcessId, IsIconic, IsWindow, PostMessageW, SW_RESTORE, SW_SHOW,
        SetForegroundWindow, ShowWindowAsync, WM_CLOSE,
    },
};

use super::application_discovery::{find_windows, validate_executable_path};

/// 通过 ApplicationSpec 管理 Windows 应用资源生命周期的无状态服务。
#[derive(Debug, Default)]
pub struct WindowsApplicationSessionProvider;

#[async_trait]
impl ApplicationSessionProvider for WindowsApplicationSessionProvider {
    async fn acquire(&self, spec: &ApplicationSpec) -> Result<AppSession, ApplicationError> {
        let spec = spec.clone();
        tokio::task::spawn_blocking(move || acquire_blocking(&spec))
            .await
            .map_err(|error| ApplicationError::LaunchFailed {
                message: format!("application resolver task failed: {error}"),
            })?
    }

    async fn cleanup(&self, session: &AppSession) -> Result<(), ApplicationError> {
        let session = session.clone();
        tokio::task::spawn_blocking(move || cleanup_blocking(&session))
            .await
            .map_err(|error| ApplicationError::CleanupFailed {
                message: format!("application cleanup task failed: {error}"),
            })?
    }
}

/// 按 AcquirePolicy 复用唯一现有应用或启动一个新进程。
fn acquire_blocking(spec: &ApplicationSpec) -> Result<AppSession, ApplicationError> {
    let executable_path = validate_executable_path(spec)?;
    if !matches!(spec.acquire_policy, AcquirePolicy::AlwaysStartNew) {
        let existing = find_windows(spec, &executable_path, None)?;
        if let Some(window) = require_unique_window(spec, existing)? {
            prepare_window(window, spec.activation_policy)?;
            return Ok(build_session(spec, &executable_path, window, false));
        }
        if matches!(spec.acquire_policy, AcquirePolicy::AttachOnly) {
            return Err(ApplicationError::NotRunning {
                description: describe_spec(spec),
            });
        }
    }
    launch_application(spec, &executable_path)
}

/// 直接启动 EXE，并只接受该子进程在超时内创建的唯一匹配窗口。
fn launch_application(
    spec: &ApplicationSpec,
    executable_path: &Path,
) -> Result<AppSession, ApplicationError> {
    let mut child = Command::new(executable_path)
        .args(&spec.arguments)
        .spawn()
        .map_err(|error| ApplicationError::LaunchFailed {
            message: format!("failed to launch '{}': {error}", executable_path.display()),
        })?;
    let process_id = child.id();
    let deadline = Instant::now() + Duration::from_millis(spec.launch_timeout_ms);
    loop {
        let candidates = match find_windows(spec, executable_path, Some(process_id)) {
            Ok(candidates) => candidates,
            Err(error) => {
                terminate_failed_acquisition(&mut child);
                return Err(error);
            }
        };
        let window = match require_unique_window(spec, candidates) {
            Ok(window) => window,
            Err(error) => {
                terminate_failed_acquisition(&mut child);
                return Err(error);
            }
        };
        if let Some(window) = window {
            if let Err(error) = prepare_window(window, spec.activation_policy) {
                terminate_failed_acquisition(&mut child);
                return Err(error);
            }
            return Ok(build_session(spec, executable_path, window, true));
        }
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => {
                terminate_failed_acquisition(&mut child);
                return Err(ApplicationError::LaunchFailed {
                    message: format!("failed to observe launched process {process_id}: {error}"),
                });
            }
        };
        if let Some(status) = status {
            return Err(ApplicationError::LaunchFailed {
                message: format!(
                    "launched process {process_id} exited with {status} before creating a matching window"
                ),
            });
        }
        if Instant::now() >= deadline {
            terminate_failed_acquisition(&mut child);
            return Err(ApplicationError::Timeout {
                timeout_ms: spec.launch_timeout_ms,
                description: describe_spec(spec),
            });
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// 获取失败时终止并回收本次工作流创建、尚未进入 ResourceTable 的子进程。
fn terminate_failed_acquisition(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// 建立只包含可验证整数身份的逻辑应用会话。
fn build_session(
    spec: &ApplicationSpec,
    executable_path: &Path,
    window: WindowIdentity,
    started_by_workflow: bool,
) -> AppSession {
    AppSession {
        id: ResourceId::new(),
        spec: spec.clone(),
        process: ProcessIdentity {
            process_id: window.process_id,
            executable_path: executable_path.to_string_lossy().into_owned(),
        },
        windows: vec![window],
        capabilities: CapabilitySet::from_iter([
            CapabilityId::WINDOWS_UIA,
            CapabilityId::VISUAL_SCREEN,
        ]),
        started_by_workflow,
    }
}

/// 按 CleanupPolicy 向仍属于原进程的窗口发送 WM_CLOSE。
fn cleanup_blocking(session: &AppSession) -> Result<(), ApplicationError> {
    let should_close = match session.spec.cleanup_policy {
        CleanupPolicy::LeaveRunning => false,
        CleanupPolicy::CloseIfStartedByWorkflow => session.started_by_workflow,
        CleanupPolicy::AlwaysClose => true,
    };
    if !should_close {
        return Ok(());
    }
    for window in &session.windows {
        let native = native_window(window.handle);
        // SAFETY: HWND 仅作为不透明身份查询，调用不解引用 Rust 内存。
        if !unsafe { IsWindow(Some(native)) }.as_bool() {
            continue;
        }
        let mut current_process_id = 0_u32;
        // SAFETY: process id 指针在同步调用期间有效且独占。
        unsafe { GetWindowThreadProcessId(native, Some(&mut current_process_id)) };
        if current_process_id != window.process_id {
            continue;
        }
        // SAFETY: 发送标准 WM_CLOSE，不携带指针载荷；已复验 HWND 所属进程。
        unsafe { PostMessageW(Some(native), WM_CLOSE, WPARAM(0), LPARAM(0)) }.map_err(|error| {
            ApplicationError::CleanupFailed {
                message: format!("failed to close application window: {error}"),
            }
        })?;
    }
    Ok(())
}

/// 强制应用作用域解析为唯一窗口，防止隐式选择多个实例。
fn require_unique_window(
    spec: &ApplicationSpec,
    windows: Vec<WindowIdentity>,
) -> Result<Option<WindowIdentity>, ApplicationError> {
    match windows.as_slice() {
        [] => Ok(None),
        [window] => Ok(Some(*window)),
        _ => Err(ApplicationError::Ambiguous {
            matches: windows.len(),
            description: describe_spec(spec),
        }),
    }
}

/// 恢复窗口，并按 ActivationPolicy 决定前台激活是否为硬条件。
fn prepare_window(
    window: WindowIdentity,
    activation: ActivationPolicy,
) -> Result<(), ApplicationError> {
    restore_window(window)?;
    if matches!(activation, ActivationPolicy::None) {
        return Ok(());
    }
    let native = native_window(window.handle);
    // SAFETY: HWND 来自刚完成的窗口枚举，调用不解引用外部内存。
    let activated = unsafe { SetForegroundWindow(native) }.as_bool();
    if !activated && matches!(activation, ActivationPolicy::Required) {
        return Err(ApplicationError::ActivationFailed {
            message: "Windows foreground lock rejected the request".to_owned(),
        });
    }
    Ok(())
}

/// 恢复最小化窗口并把恢复结果作为会话获取硬条件。
fn restore_window(window: WindowIdentity) -> Result<(), ApplicationError> {
    let native = native_window(window.handle);
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
    Err(ApplicationError::LaunchFailed {
        message: "Windows did not restore the application window".to_owned(),
    })
}

/// 把稳定整数表示恢复成 Win32 HWND 不透明值。
fn native_window(handle: u64) -> HWND {
    HWND(handle as usize as *mut std::ffi::c_void)
}

/// 生成用于错误和歧义报告的稳定应用描述。
fn describe_spec(spec: &ApplicationSpec) -> String {
    format!(
        "application(executable_path = {:?}, window_title = {:?})",
        spec.executable_path, spec.window_title
    )
}
