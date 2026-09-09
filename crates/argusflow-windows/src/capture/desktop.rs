//! 只读检测输入桌面名称及可访问状态；不切换或打开用户桌面窗口。
use windows::Win32::{Foundation::HANDLE, System::StationsAndDesktops::*};

#[derive(PartialEq, Eq)]
pub(super) enum DesktopState {
    Accessible(Vec<u16>),
    Unavailable(i32),
}
pub(super) fn current() -> DesktopState {
    // SAFETY: 只申请查询桌面名称所需权限，句柄在本函数内关闭。
    let desktop =
        match unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) } {
            Ok(desktop) => desktop,
            Err(error) => return DesktopState::Unavailable(error.code().0),
        };
    let mut name = [0_u16; 256];
    let read = unsafe {
        GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(name.as_mut_ptr().cast()),
            std::mem::size_of_val(&name) as u32,
            None,
        )
    };
    let _ = unsafe { CloseDesktop(desktop) };
    match read {
        Ok(()) => DesktopState::Accessible(
            name[..name
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(name.len())]
                .to_vec(),
        ),
        Err(error) => DesktopState::Unavailable(error.code().0),
    }
}
