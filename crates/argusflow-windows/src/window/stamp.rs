//! 随 HWND 销毁而消失的身份标记；最后一个租约释放时移除本库属性。
use crate::{WindowsError as Failure, platform::hwnd};
use argusflow_core::FailureKind;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicUsize, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::{HANDLE, HWND},
        UI::WindowsAndMessaging::{GetPropW, RemovePropW, SetPropW},
    },
    core::HSTRING,
};
static NEXT: AtomicUsize = AtomicUsize::new(1);
static LEASES: OnceLock<Mutex<HashMap<isize, Weak<Stamp>>>> = OnceLock::new();

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    handle: isize,
    token: usize,
}
impl Stamp {
    pub(crate) fn valid(&self) -> bool {
        // SAFETY: 查询整数属性，不解引用 HANDLE 的数值。
        unsafe { GetPropW(hwnd(self.handle), &name()).0 as usize == self.token && self.token != 0 }
    }
}
impl Drop for Stamp {
    fn drop(&mut self) {
        if self.valid() {
            // SAFETY: 只移除名称和值都属于本库当前租约的窗口属性；不销毁窗口。
            if let Err(error) = unsafe { RemovePropW(hwnd(self.handle), &name()) } {
                tracing::debug!(
                    hwnd = self.handle,
                    hresult = error.code().0,
                    "window identity property already unavailable"
                );
            }
        }
    }
}
fn name() -> HSTRING {
    HSTRING::from(format!("ArgusFlow.Identity.{}", std::process::id()))
}

pub(crate) fn capture(window: HWND) -> Result<Arc<Stamp>, Failure> {
    let mut leases = LEASES
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    leases.retain(|_, lease| lease.strong_count() > 0);
    let handle = window.0 as isize;
    if let Some(stamp) = leases
        .get(&handle)
        .and_then(Weak::upgrade)
        .filter(|stamp| stamp.valid())
    {
        return Ok(stamp);
    }
    if leases.len() >= 4096 {
        return Err(Failure::new(
            FailureKind::ResourceLimit,
            "window_identity",
            "窗口身份租约数量超限",
        ));
    }
    let token = NEXT
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_add(1)
        })
        .map_err(|_| {
            Failure::new(
                FailureKind::ResourceLimit,
                "window_identity",
                "窗口身份序号耗尽",
            )
        })?;
    // SAFETY: 标记为非指针整数，Windows 不解引用；UIPI 失败直接报错，不降级身份校验。
    unsafe { SetPropW(window, &name(), Some(HANDLE(token as *mut _))) }.map_err(|error| {
        Failure::new(
            FailureKind::Unavailable,
            "window_identity",
            "无法安装窗口身份标记，可能存在 UIPI 权限限制",
        )
        .with_source(error)
    })?;
    let stamp = Arc::new(Stamp { handle, token });
    leases.insert(handle, Arc::downgrade(&stamp));
    Ok(stamp)
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/window/stamp.rs"]
mod tests;
