use crate::*;
use argusflow_core::*;

pub(super) fn context() -> InspectionContext {
    InspectionContext {
        window: WindowIdentity {
            handle: 42,
            process_id: 7,
        },
        executable_path: Some("C:\\test.exe".into()),
        title: "Fixture".into(),
        class_name: "App".into(),
        bounds: InspectionRect {
            x: -1000.0,
            y: 0.0,
            width: 2000.0,
            height: 1000.0,
        },
        browser_viewport: None,
        dpi: 96,
        has_keyboard_focus: true,
    }
}

pub(super) fn entity() -> InspectedEntity {
    InspectedEntity {
        identity: "field-1".into(),
        semantics: ElementSemantics {
            role: Some(ElementRole::TextBox),
            name: Some("姓名".into()),
            automation_id: Some("NameInput".into()),
            ..Default::default()
        },
        ancestors: vec![],
        bounds: InspectionRect {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 100.0,
        },
        editable: true,
        sensitivity: FieldSensitivity::Normal,
        browser_session: None,
        page_url: None,
        confidence: 0.9,
    }
}

pub(super) fn target() -> EventEvidence {
    EventEvidence {
        screen: crate::EventScreenEvidence::default(),
        click_target: None,
        context: Some(context()),
        ui_snapshot: Some(UiSnapshot {
            backend: EvidenceBackend::Uia,
            entity: entity(),
            observed_at_ms: 10,
            observation_duration_ms: 0,
        }),
        screenshot: None,
        diagnostics: vec![],
    }
}

pub(super) fn raw(sequence: u64, input: RawInput) -> RawTraceEvent {
    RawTraceEvent {
        sequence,
        timestamp_ms: sequence as u32 * 10,
        elapsed_ms: sequence * 10,
        input,
        evidence: Some(target()),
        diagnostics: vec![],
    }
}

pub(super) struct NoCapture;
impl argusflow_core::capture::ScreenCaptureSource for NoCapture {
    fn drain(
        &self,
    ) -> Result<(Vec<argusflow_core::capture::ScreenCaptureUpdate>, bool), CaptureError> {
        Ok((Vec::new(), false))
    }
    fn sources(&self) -> Result<Vec<CaptureSourceId>, CaptureError> {
        Ok(Vec::new())
    }
    fn clock_us(&self) -> Result<u64, argusflow_core::CaptureError> {
        Ok(0)
    }
    fn poll(
        &self,
    ) -> Result<Vec<argusflow_core::capture::ScreenCaptureUpdate>, argusflow_core::CaptureError>
    {
        Err(argusflow_core::CaptureError::CaptureUnavailable {
            message: "fixture has no pixels".into(),
        })
    }
}
impl WindowEvidenceCapture for NoCapture {
    fn capture_desktop(&self) -> Result<Option<EvidenceFrame>, InspectionFailure> {
        Err(InspectionFailure::Unavailable)
    }
    fn capture(&self, _: &InspectionContext) -> Result<EvidenceFrame, InspectionFailure> {
        Err(InspectionFailure::Unavailable)
    }
}

pub(super) fn text(value: &str) -> RawInput {
    RawInput::Key {
        virtual_key: Some(65),
        flags: Some(0),
        scan_code: Some(30),
        phase: InputPhase::Down,
        text: Some(RecordedText::Plain(value.into())),
        chord: None,
    }
}
