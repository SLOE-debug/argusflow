//! 只读 UIA/CDP 证据采集；操作后截图由独立线程执行。

use crate::{EventEvidence, EvidenceBackend, RecordingDiagnostic, UiSnapshot};
use argusflow_core::{
    ElementRole, InspectedEntity, InspectionContext, InspectionFailure, InspectionProbe,
    TargetInspector, WindowIdentity, WindowInspector, capture::ScreenCaptureSource,
};
use std::{sync::Arc, time::Duration};

/// 后端只通过 core 契约协作，不依赖 OCR 或 selector 编译。
pub struct EvidenceCollector {
    /// 应用级共享桌面主机，录制停止不会释放原生采集资源。
    screen_hub: std::sync::Mutex<Option<Arc<argusflow_capture::DesktopCaptureHub>>>,
    /// 同步窗口身份查询。
    windows: Arc<dyn WindowInspector>,
    /// 已托管页面的只读观察实例。
    browser: Arc<dyn TargetInspector>,
    /// UIA 专用 worker 门面。
    uia: Arc<dyn TargetInspector>,
    /// 独立采样线程使用的同步像素能力。
    capture: Arc<dyn ScreenCaptureSource>,
}

impl EvidenceCollector {
    pub(crate) fn pixel_differ(
        &self,
    ) -> Result<
        Box<dyn argusflow_core::capture::refinement::PixelDiffer>,
        argusflow_core::CaptureError,
    > {
        self.capture.create_pixel_differ()
    }
    pub(crate) fn shutdown_capture(&self) -> Result<(), argusflow_core::CaptureFailure> {
        let hub = self
            .screen_hub
            .lock()
            .map_err(|_| argusflow_core::CaptureFailure::Unavailable)?
            .take();
        if let Some(hub) = hub {
            hub.shutdown()?;
        }
        Ok(())
    }
    pub(crate) fn screen_hub(
        &self,
        scheduler: Arc<dyn argusflow_capture::CaptureScheduler>,
    ) -> Result<Arc<argusflow_capture::DesktopCaptureHub>, argusflow_core::CaptureFailure> {
        let mut hub = self
            .screen_hub
            .lock()
            .map_err(|_| argusflow_core::CaptureFailure::Unavailable)?;
        if hub.is_none() {
            *hub = Some(argusflow_capture::DesktopCaptureHub::start(
                self.capture.clone(),
                scheduler,
            )?);
        }
        hub.as_ref()
            .cloned()
            .ok_or(argusflow_core::CaptureFailure::Unavailable)
    }
    pub(crate) fn screen_clock_us(&self) -> Result<u64, argusflow_core::CaptureError> {
        self.capture.clock_us()
    }
    /// 复用宿主结构化观察实例，截图使用独立快速能力。
    pub fn new(
        windows: Arc<dyn WindowInspector>,
        browser: Arc<dyn TargetInspector>,
        uia: Arc<dyn TargetInspector>,
        capture: Arc<dyn ScreenCaptureSource>,
    ) -> Self {
        Self {
            screen_hub: std::sync::Mutex::new(None),
            windows,
            browser,
            uia,
            capture,
        }
    }

    /// 在事件摄入阶段读取真实点击窗口或键盘上下文。
    pub fn context(&self, probe: InspectionProbe) -> Result<InspectionContext, InspectionFailure> {
        self.windows.context(probe)
    }

    /// 窗口事件必须使用事件携带的身份。
    pub(crate) fn window_context(
        &self,
        window: WindowIdentity,
    ) -> Result<InspectionContext, InspectionFailure> {
        self.windows.window_context(window)
    }

    /// 尝试结构化证据；不可用时返回窗口事实，图像由调用方保留。
    pub async fn collect(
        &self,
        context: InspectionContext,
        probe: InspectionProbe,
        observed_at_ms: u64,
        event_elapsed_ms: u64,
    ) -> EventEvidence {
        let started = tokio::time::Instant::now();
        let mut evidence = EventEvidence {
            context: Some(context.clone()),
            ..Default::default()
        };
        for (backend, inspector, timeout) in [
            (
                EvidenceBackend::ManagedCdp,
                &self.browser,
                Duration::from_millis(150),
            ),
            (EvidenceBackend::Uia, &self.uia, Duration::from_millis(1000)),
        ] {
            if observed_at_ms.saturating_sub(event_elapsed_ms)
                + started.elapsed().as_millis() as u64
                >= 150
            {
                evidence
                    .diagnostics
                    .push(RecordingDiagnostic::LateInspection);
                break;
            }
            let provider_started_ms = observed_at_ms + started.elapsed().as_millis() as u64;
            let provider_started = tokio::time::Instant::now();
            // 操作后截图独立调度，不受 provider 等待影响。
            // UIA 外层预算须大于自身 800ms 恢复预算，避免提前取消导致阻塞 worker 无法恢复。
            let result = tokio::time::timeout(timeout, inspector.inspect(&context, probe))
                .await
                .unwrap_or(Err(InspectionFailure::Timeout));
            match result {
                Ok(entity) if reliable(&entity, probe) => {
                    let current = if matches!(probe, InspectionProbe::Window) {
                        self.window_context(context.window)
                    } else {
                        self.context(probe)
                    };
                    if !current.is_ok_and(|current| {
                        current.window == context.window && current.bounds == context.bounds
                    }) {
                        evidence
                            .diagnostics
                            .push(RecordingDiagnostic::InspectionFailed {
                                backend,
                                reason: InspectionFailure::ContextChanged,
                            });
                        break;
                    }
                    evidence.ui_snapshot = Some(UiSnapshot {
                        backend,
                        entity,
                        observed_at_ms: provider_started_ms,
                        observation_duration_ms: provider_started.elapsed().as_millis() as u64,
                    });
                    break;
                }
                Ok(_) => evidence
                    .diagnostics
                    .push(RecordingDiagnostic::InspectionFailed {
                        backend,
                        reason: InspectionFailure::NoElement,
                    }),
                Err(reason) => evidence
                    .diagnostics
                    .push(RecordingDiagnostic::InspectionFailed { backend, reason }),
            }
        }
        evidence
    }
}

/// 只命中窗口/容器或无角色不能证明具有可靠的结构化交互信息。
fn reliable(entity: &InspectedEntity, probe: InspectionProbe) -> bool {
    entity.bounds.is_valid()
        && entity.confidence.is_finite()
        && entity.confidence >= 0.5
        && (matches!(probe, InspectionProbe::Window)
            || !matches!(
                entity.semantics.role,
                None | Some(ElementRole::Window | ElementRole::Pane | ElementRole::Document)
            ))
        && match probe {
            InspectionProbe::Point(point) => entity.bounds.contains(point),
            InspectionProbe::Focus | InspectionProbe::Window => true,
        }
}
