//! 截图编排：最小身份复验、互斥复用桌面复制会话，返回独立原生像素。

use super::{evidence_desktop::EvidenceDesktop, evidence_geometry};
use argusflow_core::{EvidenceFrame, InspectionContext, InspectionFailure, WindowEvidenceCapture};
use std::sync::Mutex;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};

/// 采集用户可见窗口区域；不执行颜色转换、PNG 编码或 OCR。
#[derive(Default)]
pub struct WindowsEventCapture {
    /// immediate context 的全部调用由互斥锁串行化；资源随实例释放，不放进 TLS 析构。
    desktop: Mutex<Option<EvidenceDesktop>>,
}

impl WindowEvidenceCapture for WindowsEventCapture {
    fn capture(&self, context: &InspectionContext) -> Result<EvidenceFrame, InspectionFailure> {
        let _dpi = PhysicalDpiScope::enter();
        evidence_geometry::validate(context)?;
        let bounds = evidence_geometry::visible_bounds(context.bounds)?;
        let frame = {
            let mut desktop = self
                .desktop
                .lock()
                .map_err(|_| InspectionFailure::Unavailable)?;
            if desktop.is_none() {
                *desktop = Some(EvidenceDesktop::new()?);
            }
            let result = desktop
                .as_mut()
                .ok_or(InspectionFailure::Unavailable)?
                .capture(bounds);
            // 桌面切换、热插拔或设备故障均撤销缓存；下次事件重建，故障时不返回旧画面。
            if result.is_err() {
                *desktop = None;
            }
            result
        }?;
        evidence_geometry::validate(context)?;
        Ok(frame)
    }
}

/// 同步查询与截图的 DPI 边界，不跨 await，也不修改调用方后续线程状态。
struct PhysicalDpiScope(DPI_AWARENESS_CONTEXT);
impl PhysicalDpiScope {
    fn enter() -> Self {
        // SAFETY: 仅改变当前线程，Drop 恢复原值。
        Self(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) })
    }
}
impl Drop for PhysicalDpiScope {
    fn drop(&mut self) {
        if !self.0.0.is_null() {
            unsafe { SetThreadDpiAwarenessContext(self.0) };
        }
    }
}
