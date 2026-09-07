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
    fn context(&self, _: InspectionProbe) -> Result<InspectionContext, InspectionFailure> {
        Ok(context())
    }
}

struct Inspector {
    label: ResolutionBackend,
    calls: Arc<Mutex<Vec<ResolutionBackend>>>,
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
    let backends = [
        ResolutionBackend::ManagedCdp,
        ResolutionBackend::Uia,
        ResolutionBackend::Vision,
    ];
    for succeeds in 0..=3 {
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
        let resolver = TargetResolver::new(
            Arc::new(Windows),
            inspectors[0].clone(),
            inspectors[1].clone(),
            inspectors[2].clone(),
        );
        let target = resolver
            .resolve(
                context(),
                InspectionProbe::Point(ScreenPoint { x: 20, y: 20 }),
            )
            .await;
        assert_eq!(*calls.lock().unwrap(), backends[..(succeeds + 1).min(3)]);
        assert_eq!(
            target.backend,
            backends
                .get(succeeds)
                .copied()
                .unwrap_or(ResolutionBackend::Coordinate)
        );
        assert_eq!(
            target
                .diagnostics
                .iter()
                .filter(|item| matches!(item, RecordingDiagnostic::Fallback { .. }))
                .count(),
            succeeds
        );
    }
}

#[tokio::test(start_paused = true)]
async fn managed_backend_timeout_falls_back_and_sensitive_names_are_removed() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let browser = Arc::new(Inspector {
        label: ResolutionBackend::ManagedCdp,
        calls: calls.clone(),
        result: Ok(entity()),
        delay: Duration::from_secs(60),
    });
    let mut sensitive = entity();
    sensitive.sensitivity = FieldSensitivity::Sensitive;
    sensitive.semantics.name = Some("SECRET_INPUT_VALUE".into());
    let uia = Arc::new(Inspector {
        label: ResolutionBackend::Uia,
        calls: calls.clone(),
        result: Ok(sensitive),
        delay: Duration::ZERO,
    });
    let vision = Arc::new(Inspector {
        label: ResolutionBackend::Vision,
        calls,
        result: Ok(entity()),
        delay: Duration::ZERO,
    });
    let resolver = TargetResolver::new(Arc::new(Windows), browser, uia, vision);
    let target = resolver.resolve(context(), InspectionProbe::Focus).await;
    assert_eq!(target.backend, ResolutionBackend::Uia);
    assert_eq!(
        target.diagnostics[0],
        RecordingDiagnostic::Fallback {
            backend: ResolutionBackend::ManagedCdp,
            reason: InspectionFailure::Timeout
        }
    );
    assert!(
        !serde_json::to_string(&target)
            .unwrap()
            .contains("SECRET_INPUT_VALUE")
    );
}

#[tokio::test]
async fn replaced_window_is_never_accepted_as_the_original_target() {
    struct ChangedWindow;
    impl WindowInspector for ChangedWindow {
        fn context(&self, _: InspectionProbe) -> Result<InspectionContext, InspectionFailure> {
            let mut changed = context();
            changed.window.process_id += 1;
            Ok(changed)
        }
    }
    let inspector = Arc::new(Inspector {
        label: ResolutionBackend::Uia,
        calls: Arc::new(Mutex::new(vec![])),
        result: Ok(entity()),
        delay: Duration::ZERO,
    });
    let resolver = TargetResolver::new(
        Arc::new(ChangedWindow),
        inspector.clone(),
        inspector.clone(),
        inspector,
    );
    let target = resolver.resolve(context(), InspectionProbe::Focus).await;
    assert_eq!(target.backend, ResolutionBackend::Coordinate);
    assert!(target.entity.is_none());
}
