use super::fixtures::{context, entity};
use crate::*;
use argusflow_core::*;
use async_trait::async_trait;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

struct Windows;
impl WindowInspector for Windows {
    fn window_context(&self, _: WindowIdentity) -> Result<InspectionContext, InspectionFailure> {
        Ok(context())
    }
    fn context(&self, _: InspectionProbe) -> Result<InspectionContext, InspectionFailure> {
        Ok(context())
    }
}

struct Inspector {
    label: EvidenceBackend,
    calls: Arc<Mutex<Vec<EvidenceBackend>>>,
    result: Result<InspectedEntity, InspectionFailure>,
    delay: Duration,
}

#[async_trait]
impl TargetInspector for Inspector {
    async fn inspect(
        &self,
        _: &InspectionContext,
        _: InspectionProbe,
    ) -> Result<InspectedEntity, InspectionFailure> {
        self.calls.lock().unwrap().push(self.label);
        tokio::time::sleep(self.delay).await;
        self.result.clone()
    }
}

#[tokio::test]
async fn each_success_stops_the_ordered_fallback_chain() {
    let backends = [EvidenceBackend::ManagedCdp, EvidenceBackend::Uia];
    for succeeds in 0..=2 {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let inspectors: Vec<Arc<dyn TargetInspector>> = backends
            .iter()
            .enumerate()
            .map(|(index, backend)| {
                Arc::new(Inspector {
                    label: *backend,
                    calls: calls.clone(),
                    delay: Duration::ZERO,
                    result: if index == succeeds {
                        Ok(entity())
                    } else {
                        Err(InspectionFailure::NoElement)
                    },
                }) as Arc<dyn TargetInspector>
            })
            .collect();
        let resolver = EvidenceCollector::new(
            Arc::new(Windows),
            inspectors[0].clone(),
            inspectors[1].clone(),
            Arc::new(super::fixtures::NoCapture),
        );
        let target = resolver
            .collect(
                context(),
                InspectionProbe::Point(ScreenPoint { x: 20, y: 20 }),
                10,
                10,
            )
            .await;
        assert_eq!(*calls.lock().unwrap(), backends[..(succeeds + 1).min(2)]);
        assert_eq!(
            target.ui_snapshot.map(|snapshot| snapshot.backend),
            backends.get(succeeds).copied()
        );
        assert_eq!(
            target
                .diagnostics
                .iter()
                .filter(|item| matches!(item, RecordingDiagnostic::InspectionFailed { .. }))
                .count(),
            succeeds
        );
    }
}

#[tokio::test(start_paused = true)]
async fn managed_backend_timeout_is_bounded_and_raw_sensitive_names_are_retained() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let browser = Arc::new(Inspector {
        label: EvidenceBackend::ManagedCdp,
        calls: calls.clone(),
        result: Ok(entity()),
        delay: Duration::from_secs(60),
    });
    let mut sensitive = entity();
    sensitive.sensitivity = FieldSensitivity::Sensitive;
    sensitive.semantics.name = Some("SECRET_INPUT_VALUE".into());
    let uia = Arc::new(Inspector {
        label: EvidenceBackend::Uia,
        calls: calls.clone(),
        result: Ok(sensitive),
        delay: Duration::ZERO,
    });
    let resolver = EvidenceCollector::new(
        Arc::new(Windows),
        browser,
        uia,
        Arc::new(super::fixtures::NoCapture),
    );
    let target = resolver
        .collect(context(), InspectionProbe::Focus, 10, 10)
        .await;
    assert!(target.ui_snapshot.is_none());
    assert!(
        target
            .diagnostics
            .contains(&RecordingDiagnostic::LateInspection)
    );
    assert_eq!(
        target.diagnostics[0],
        RecordingDiagnostic::InspectionFailed {
            backend: EvidenceBackend::ManagedCdp,
            reason: InspectionFailure::Timeout
        }
    );
    assert!(
        !serde_json::to_string(&target)
            .unwrap()
            .contains("SECRET_INPUT_VALUE")
    );
    // 快速失败可以进入 UIA；查询耗尽事件预算后禁止用稍后的 UIA 冒充事件快照。
    let failed = Arc::new(Inspector {
        label: EvidenceBackend::ManagedCdp,
        calls: calls.clone(),
        result: Err(InspectionFailure::UnmanagedWindow),
        delay: Duration::ZERO,
    });
    let mut sensitive = entity();
    sensitive.sensitivity = FieldSensitivity::Sensitive;
    sensitive.semantics.name = Some("SECRET_INPUT_VALUE".into());
    let uia = Arc::new(Inspector {
        label: EvidenceBackend::Uia,
        calls,
        result: Ok(sensitive),
        delay: Duration::ZERO,
    });
    let collector = EvidenceCollector::new(
        Arc::new(Windows),
        failed,
        uia,
        Arc::new(super::fixtures::NoCapture),
    );
    let evidence = collector
        .collect(context(), InspectionProbe::Focus, 10, 10)
        .await;
    assert_eq!(
        evidence.ui_snapshot.as_ref().unwrap().backend,
        EvidenceBackend::Uia
    );
    assert!(
        serde_json::to_string(&evidence)
            .unwrap()
            .contains("SECRET_INPUT_VALUE")
    );
}

#[tokio::test]
async fn replaced_window_is_never_accepted_as_the_original_target() {
    struct ChangedWindow;
    impl WindowInspector for ChangedWindow {
        fn window_context(
            &self,
            _: WindowIdentity,
        ) -> Result<InspectionContext, InspectionFailure> {
            self.context(InspectionProbe::Focus)
        }
        fn context(&self, _: InspectionProbe) -> Result<InspectionContext, InspectionFailure> {
            let mut changed = context();
            changed.window.process_id += 1;
            Ok(changed)
        }
    }
    let inspector = Arc::new(Inspector {
        label: EvidenceBackend::Uia,
        calls: Arc::new(Mutex::new(vec![])),
        result: Ok(entity()),
        delay: Duration::ZERO,
    });
    let resolver = EvidenceCollector::new(
        Arc::new(ChangedWindow),
        inspector.clone(),
        inspector.clone(),
        Arc::new(super::fixtures::NoCapture),
    );
    let target = resolver
        .collect(context(), InspectionProbe::Focus, 10, 10)
        .await;
    assert!(target.ui_snapshot.is_none());
}
