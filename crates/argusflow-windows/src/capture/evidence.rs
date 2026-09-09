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
    /// 来源重建代数，旧增量不能应用到新设备或新拓扑。
    generation: std::sync::atomic::AtomicU64,
}

impl argusflow_core::capture::ScreenCaptureSource for WindowsEventCapture {
    fn create_pixel_differ(
        &self,
    ) -> Result<
        Box<dyn argusflow_core::capture::refinement::PixelDiffer>,
        argusflow_core::CaptureError,
    > {
        Ok(Box::new(super::gpu_difference::GpuDifference::new()?))
    }
    fn drain(
        &self,
    ) -> Result<
        (Vec<argusflow_core::capture::ScreenCaptureUpdate>, bool),
        argusflow_core::CaptureError,
    > {
        let mut desktop = self.desktop.lock().map_err(|_| capture_failure())?;
        desktop
            .as_mut()
            .ok_or_else(capture_failure)?
            .drain_updates(argusflow_core::CaptureGeneration(
                self.generation.load(std::sync::atomic::Ordering::Relaxed),
            ))
            .map_err(|_| capture_failure())
    }
    fn sources(
        &self,
    ) -> Result<Vec<argusflow_core::CaptureSourceId>, argusflow_core::CaptureError> {
        let _dpi = PhysicalDpiScope::enter();
        let mut desktop = self.desktop.lock().map_err(|_| capture_failure())?;
        if desktop.is_none() {
            *desktop = Some(EvidenceDesktop::new().map_err(|_| capture_failure())?);
            self.generation
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(desktop.as_mut().ok_or_else(capture_failure)?.sources())
    }
    fn clock_us(&self) -> Result<u64, argusflow_core::CaptureError> {
        super::clock::now_us().map_err(|_| capture_failure())
    }
    fn poll(
        &self,
    ) -> Result<Vec<argusflow_core::capture::ScreenCaptureUpdate>, argusflow_core::CaptureError>
    {
        use std::sync::atomic::Ordering;
        let _dpi = PhysicalDpiScope::enter();
        let mut desktop = self.desktop.lock().map_err(|_| capture_failure())?;
        if desktop.is_none() {
            *desktop = Some(EvidenceDesktop::new().map_err(|_| capture_failure())?);
            self.generation.fetch_add(1, Ordering::Relaxed);
        }
        let result = desktop.as_mut().ok_or_else(capture_failure)?.poll_updates(
            argusflow_core::CaptureGeneration(self.generation.load(Ordering::Relaxed)),
        );
        if result.is_err() {
            *desktop = None;
        }
        result.map_err(|_| capture_failure())
    }
}

fn capture_failure() -> argusflow_core::CaptureError {
    argusflow_core::CaptureError::CaptureUnavailable {
        message: "desktop capture source is unavailable".into(),
    }
}

impl WindowEvidenceCapture for WindowsEventCapture {
    fn capture_desktop(&self) -> Result<Option<EvidenceFrame>, InspectionFailure> {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
        };
        let _dpi = PhysicalDpiScope::enter();
        // SAFETY: 只读虚拟桌面物理范围，DPI scope 在同一线程恢复。
        let bounds = unsafe {
            argusflow_core::InspectionRect {
                x: GetSystemMetrics(SM_XVIRTUALSCREEN).into(),
                y: GetSystemMetrics(SM_YVIRTUALSCREEN).into(),
                width: GetSystemMetrics(SM_CXVIRTUALSCREEN).into(),
                height: GetSystemMetrics(SM_CYVIRTUALSCREEN).into(),
            }
        };
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
            .capture_update(bounds, true);
        if result.is_err() {
            *desktop = None;
        }
        result
    }
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
