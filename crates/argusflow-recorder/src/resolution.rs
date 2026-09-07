//! Physical → Semantic 单向解析链；所有适配器通过 core 的只读契约装配。

use crate::{RecordingDiagnostic, ResolutionBackend, ResolvedTarget, synthesize_selectors};
use argusflow_core::{
    FieldSensitivity, InspectionContext, InspectionFailure, InspectionProbe, TargetInspector,
    WindowInspector,
};
use std::{sync::Arc, time::Duration};

/// 宿主共享现有 browser/UIA/Vision 实例，录制器不依赖具体后端 crate。
pub struct TargetResolver {
    /// 实际点所在窗口的同步只读探测。
    windows: Arc<dyn WindowInspector>,
    /// 只有当前 runtime 已附加的页面可返回成功。
    browser: Arc<dyn TargetInspector>,
    /// UIA 原有 MTA worker。
    uia: Arc<dyn TargetInspector>,
    /// 共用窗口捕获与 OCR Scene。
    vision: Arc<dyn TargetInspector>,
}

impl TargetResolver {
    /// 建立唯一解析顺序；外部不能修改内部 backend 集合。
    pub fn new(
        windows: Arc<dyn WindowInspector>,
        browser: Arc<dyn TargetInspector>,
        uia: Arc<dyn TargetInspector>,
        vision: Arc<dyn TargetInspector>,
    ) -> Self {
        Self {
            windows,
            browser,
            uia,
            vision,
        }
    }

    /// 仅由 ingestion worker 调用，在开始慢速语义检查前冻结窗口。
    pub fn context(&self, probe: InspectionProbe) -> Result<InspectionContext, InspectionFailure> {
        self.windows.context(probe)
    }

    /// 明确 late/context failure 的 coordinate 降级，不尝试重新解释历史屏幕。
    pub fn fallback(
        context: Option<InspectionContext>,
        probe: InspectionProbe,
        diagnostics: Vec<RecordingDiagnostic>,
    ) -> ResolvedTarget {
        let point = match probe {
            InspectionProbe::Point(point) => Some(point),
            InspectionProbe::Focus => None,
        };
        let candidates = synthesize_selectors(None, ResolutionBackend::Coordinate, point);
        ResolvedTarget {
            context,
            backend: ResolutionBackend::Coordinate,
            entity: None,
            preferred_selector: (!candidates.is_empty()).then_some(0),
            selector_candidates: candidates,
            confidence: 0.05,
            diagnostics,
        }
    }

    /// CDP → UIA → Vision → coordinate；任何 provider 故障都进入结构化诊断。
    pub async fn resolve(
        &self,
        context: InspectionContext,
        probe: InspectionProbe,
    ) -> ResolvedTarget {
        let mut diagnostics = Vec::new();
        for (backend, inspector, timeout) in [
            (
                ResolutionBackend::ManagedCdp,
                &self.browser,
                Duration::from_millis(800),
            ),
            (
                ResolutionBackend::Uia,
                &self.uia,
                Duration::from_millis(1500),
            ),
            (
                ResolutionBackend::Vision,
                &self.vision,
                Duration::from_secs(8),
            ),
        ] {
            let result = tokio::time::timeout(timeout, inspector.inspect(&context, probe))
                .await
                .unwrap_or(Err(InspectionFailure::Timeout));
            match result {
                Ok(mut entity)
                    if entity.bounds.is_valid()
                        && entity.confidence.is_finite()
                        && entity.confidence > 0.0 =>
                {
                    // 拒绝 provider 完成期间被替换/移走的窗口；不能把新窗口的事实套在旧事件上。
                    if !self.context(probe).is_ok_and(|current| {
                        current.window == context.window && current.bounds == context.bounds
                    }) {
                        diagnostics.push(RecordingDiagnostic::Fallback {
                            backend,
                            reason: InspectionFailure::ContextChanged,
                        });
                        return Self::fallback(Some(context), probe, diagnostics);
                    }
                    if entity.sensitivity == FieldSensitivity::Sensitive {
                        entity.semantics.name = None;
                        entity
                            .ancestors
                            .iter_mut()
                            .for_each(|item| item.name = None);
                        diagnostics.push(RecordingDiagnostic::Redacted);
                    }
                    let point = match probe {
                        InspectionProbe::Point(point) => Some(point),
                        InspectionProbe::Focus => None,
                    };
                    let candidates = synthesize_selectors(Some(&entity), backend, point);
                    diagnostics.push(RecordingDiagnostic::SelectorUniquenessUnverified);
                    return ResolvedTarget {
                        context: Some(context),
                        backend,
                        confidence: entity.confidence.clamp(0.0, 1.0),
                        entity: Some(entity),
                        preferred_selector: (!candidates.is_empty()).then_some(0),
                        selector_candidates: candidates,
                        diagnostics,
                    };
                }
                Ok(_) => diagnostics.push(RecordingDiagnostic::Fallback {
                    backend,
                    reason: InspectionFailure::NoElement,
                }),
                Err(reason) => diagnostics.push(RecordingDiagnostic::Fallback { backend, reason }),
            }
        }
        Self::fallback(Some(context), probe, diagnostics)
    }
}
