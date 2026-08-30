//! 视觉表面窗口与可激活 owner 窗口之间的显式降级策略。

use argusflow_agent::{MaterializedTarget, WindowContext};
use thiserror::Error;

use super::keyboard::{KeyboardInputError, ensure_foreground_window};

/// 成功获得前台权限的窗口来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActivatedVisualWindow<'window> {
    /// OCR surface 自身可直接成为前台窗口。
    Surface(&'window WindowContext),
    /// surface 拒绝激活后改用其显式 owner。
    OwnerFallback(&'window WindowContext),
}

impl ActivatedVisualWindow<'_> {
    /// 返回注入前必须持续保持为前台的窗口。
    pub(super) const fn window(&self) -> &WindowContext {
        match self {
            Self::Surface(window) | Self::OwnerFallback(window) => window,
        }
    }

    /// 返回本次是否由 owner 窗口取得前台权限。
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

/// 优先激活 OCR 命中的 surface；仅在前台锁拒绝时尝试其显式 owner。
pub(super) fn activate_visual_target(
    target: &MaterializedTarget,
) -> Result<ActivatedVisualWindow<'_>, VisualWindowActivationError> {
    activate_with(
        &target.window,
        target.activation_fallback.as_ref(),
        ensure_foreground_window,
    )
}

/// 将窗口选择策略与 Win32 副作用分离，确保 fallback 条件可被纯单元测试覆盖。
fn activate_with<'window>(
    surface: &'window WindowContext,
    owner_fallback: Option<&'window WindowContext>,
    mut activate: impl FnMut(&WindowContext) -> Result<(), KeyboardInputError>,
) -> Result<ActivatedVisualWindow<'window>, VisualWindowActivationError> {
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

        let activated = activate_with(&surface, Some(&owner), |candidate| {
            attempts.push(candidate.handle);
            Ok(())
        })
        .expect("surface should activate directly");

        assert_eq!(activated, ActivatedVisualWindow::Surface(&surface));
        assert_eq!(attempts, vec![10]);
    }

    #[test]
    fn foreground_rejection_falls_back_to_owner() {
        let surface = window(10);
        let owner = window(20);
        let mut attempts = Vec::new();

        let activated = activate_with(&surface, Some(&owner), |candidate| {
            attempts.push(candidate.handle);
            if candidate.handle == surface.handle {
                Err(KeyboardInputError::ActivationFailed)
            } else {
                Ok(())
            }
        })
        .expect("owner should provide foreground permission");

        assert_eq!(activated, ActivatedVisualWindow::OwnerFallback(&owner));
        assert_eq!(attempts, vec![10, 20]);
    }

    #[test]
    fn invalid_surface_identity_never_falls_back() {
        let surface = window(10);
        let owner = window(20);
        let mut attempts = Vec::new();

        let error = activate_with(&surface, Some(&owner), |candidate| {
            attempts.push(candidate.handle);
            Err(KeyboardInputError::InvalidWindow)
        })
        .expect_err("invalid HWND must not redirect input through an owner");

        assert!(matches!(
            error,
            VisualWindowActivationError::Surface {
                source: KeyboardInputError::InvalidWindow
            }
        ));
        assert_eq!(attempts, vec![10]);
    }
}
