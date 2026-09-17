//! 本线程COM/MF初始化配对，必须晚于所有媒体对象释放。
use super::model::Result;
use windows::Win32::{Media::MediaFoundation::*, System::Com::*};
pub(super) struct Runtime;
impl Runtime {
    pub fn new() -> Result<Self> {
        // SAFETY: 本同步入口要求专用 MTA 线程，配对初始化与释放。
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
                CoUninitialize();
                return Err(error.into());
            }
        }
        Ok(Self)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        // SAFETY: 会话对象及工作线程已经释放，配对本线程初始化。
        unsafe {
            let _ = MFShutdown();
            CoUninitialize();
        }
    }
}
