//! 视觉表面窗口与可激活 owner 窗口之间的显式降级策略。

use argusflow_agent::{MaterializedTarget, WindowContext};
use thiserror::Error;

use super::keyboard::{KeyboardInputError, ensure_foreground_window, is_foreground_window};

/// 成功获得前台权限的窗口来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActivatedVisualWindow<'window> {
    /// OCR surface 自身可直接成为前台窗口。
    Surface(&'window WindowContext),
    /// 使用已经位于前台或在 surface 拒绝后成功激活的显式 owner。
    OwnerFallback(&'window WindowContext),
}

impl ActivatedVisualWindow<'_> {
    /// 返回注入前必须持续保持为前台的窗口。
    pub(super) const fn window(&self) -> &WindowContext {
        match self {
            Self::Surface(window) | Self::OwnerFallback(window) => window,
        }
    }

    /// 返回本次是否沿用或取得了 owner 窗口的前台权限。
    pub(super) const fn used_owner_fallback(&self) -> bool {
        matches!(self, Self::OwnerFallback(_))
    }
}

/// 视觉点击无法获得安全前台窗口。
#[derive(Debug, Error)]
pub(super) enum VisualWindowActivationError {
    /// 表面窗口发生身份错误，或拒绝激活且没有 owner fallback。
    #[error("视觉目标窗口无法激活: {source}")]
    Surface {
        /// 表面 HWND 的结构化失败原因。
        source: KeyboardInputError,
    },
    /// 表面只因前台锁失败而进入 owner 降级，但 owner 同样无法激活。
    #[error("视觉目标弹窗无法激活，且 owner 窗口降级失败: {source}")]
    OwnerFallback {
        /// owner HWND 的结构化失败原因。
        source: KeyboardInputError,
    },
}

/// owner 已经位于前台时保持现状，避免激活 owned popup 的副作用关闭瞬态表面。
pub(super) fn activate_visual_target(
    target: &MaterializedTarget,
) -> Result<ActivatedVisualWindow<'_>, VisualWindowActivationError> {
    activate_with(
        &target.window,
        target.activation_fallback.as_ref(),
        is_foreground_window,
        ensure_foreground_window,
    )
}

/// 将窗口选择策略与 Win32 副作用分离，确保 fallback 条件可被纯单元测试覆盖。
fn activate_with<'window>(
    surface: &'window WindowContext,
    owner_fallback: Option<&'window WindowContext>,
    mut is_foreground: impl FnMut(&WindowContext) -> bool,
    mut activate: impl FnMut(&WindowContext) -> Result<(), KeyboardInputError>,
) -> Result<ActivatedVisualWindow<'window>, VisualWindowActivationError> {
    // 微信等桌面应用会把搜索结果实现为不可前台化的 owned popup；其 owner 已经在前台时，
    // 任何 SetForegroundWindow 尝试都可能触发失焦关闭，因此直接沿用 owner 的输入权限。
    if let Some(owner) = owner_fallback {
        if is_foreground(owner) {
            return Ok(ActivatedVisualWindow::OwnerFallback(owner));
        }
    }
    match activate(surface) {
        Ok(()) => Ok(ActivatedVisualWindow::Surface(surface)),
        Err(KeyboardInputError::ActivationFailed) => {
            let Some(owner) = owner_fallback else {
                return Err(VisualWindowActivationError::Surface {
                    source: KeyboardInputError::ActivationFailed,
                });
            };
            activate(owner)
                .map_err(|source| VisualWindowActivationError::OwnerFallback { source })?;
            Ok(ActivatedVisualWindow::OwnerFallback(owner))
        }
        Err(source) => Err(VisualWindowActivationError::Surface { source }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建只包含 HWND/PID 的测试窗口身份。
    const fn window(handle: u64) -> WindowContext {
        WindowContext {
            handle,
            process_id: 7,
        }
    }

    #[test]
    fn direct_surface_activation_does_not_touch_owner() {
        let surface = window(10);
        let owner = window(20);
        let mut attempts = Vec::new();

        let activated = activate_with(
            &surface,
            Some(&owner),
            |_| false,
            |candidate| {
                attempts.push(candidate.handle);
                Ok(())
            },
        )
        .expect("surface should activate directly");

        assert_eq!(activated, ActivatedVisualWindow::Surface(&surface));
        assert_eq!(attempts, vec![10]);
    }

    #[test]
    fn foreground_rejection_falls_back_to_owner() {
        let surface = window(10);
        let owner = window(20);
        let mut attempts = Vec::new();

        let activated = activate_with(
            &surface,
            Some(&owner),
            |_| false,
            |candidate| {
                attempts.push(candidate.handle);
                if candidate.handle == surface.handle {
                    Err(KeyboardInputError::ActivationFailed)
                } else {
                    Ok(())
                }
            },
        )
        .expect("owner should provide foreground permission");

        assert_eq!(activated, ActivatedVisualWindow::OwnerFallback(&owner));
        assert_eq!(attempts, vec![10, 20]);
    }

    #[test]
    fn invalid_surface_identity_never_falls_back() {
        let surface = window(10);
        let owner = window(20);
        let mut attempts = Vec::new();

        let error = activate_with(
            &surface,
            Some(&owner),
            |_| false,
            |candidate| {
                attempts.push(candidate.handle);
                Err(KeyboardInputError::InvalidWindow)
            },
        )
        .expect_err("invalid HWND must not redirect input through an owner");

        assert!(matches!(
            error,
            VisualWindowActivationError::Surface {
                source: KeyboardInputError::InvalidWindow
            }
        ));
        assert_eq!(attempts, vec![10]);
    }

    #[test]
    fn foreground_owner_skips_surface_activation() {
        let surface = window(10);
        let owner = window(20);
        let mut attempts = Vec::new();

        let activated = activate_with(
            &surface,
            Some(&owner),
            |candidate| candidate.handle == owner.handle,
            |candidate| {
                attempts.push(candidate.handle);
                Ok(())
            },
        )
        .expect("foreground owner should already provide safe input permission");

        assert_eq!(activated, ActivatedVisualWindow::OwnerFallback(&owner));
        assert!(
            attempts.is_empty(),
            "no activation call may dismiss the popup"
        );
    }
}
