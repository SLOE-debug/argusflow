//! 实际采样线程、PNG 写入和独立 provider 的集成回归，屏幕由内存 fixture 提供。
use crate::*;
use argusflow_core::*;
use async_trait::async_trait;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

struct DelayedDesktop(Instant);
impl WindowInspector for DelayedDesktop {
    fn context(&self, _: InspectionProbe) -> Result<InspectionContext, InspectionFailure> {
        let drawn = self.0.elapsed() >= Duration::from_millis(750);
        Ok(InspectionContext {
            window: WindowIdentity {
                handle: if drawn { 2 } else { 1 },
                process_id: 10,
            },
            executable_path: None,
            title: if drawn { "Search" } else { "Desktop" }.into(),
            class_name: "fixture".into(),
            bounds: InspectionRect {
                x: -2.0,
                y: 0.0,
                width: 4.0,
                height: 4.0,
            },
            browser_viewport: None,
            dpi: 96,
            has_keyboard_focus: drawn,
        })
    }
    fn window_context(&self, _: WindowIdentity) -> Result<InspectionContext, InspectionFailure> {
        self.context(InspectionProbe::Window)
    }
}
impl WindowEvidenceCapture for DelayedDesktop {
    fn capture_desktop(&self) -> Result<Option<EvidenceFrame>, InspectionFailure> {
        self.capture(&self.context(InspectionProbe::Focus)?)
            .map(Some)
    }
    fn capture(&self, context: &InspectionContext) -> Result<EvidenceFrame, InspectionFailure> {
        EvidenceFrame::new(
            context.bounds,
            4,
            4,
            EvidencePixelFormat::Rgba8,
            vec![if context.window.handle == 2 { 255 } else { 0 }; 64],
        )
    }
}
#[async_trait]
impl TargetInspector for DelayedDesktop {
    async fn inspect(
        &self,
        _: &InspectionContext,
        _: InspectionProbe,
    ) -> Result<InspectedEntity, InspectionFailure> {
        panic!("操作后采样不得等待或调用结构化 provider")
    }
}

#[tokio::test]
async fn delayed_search_primary_png_is_post_action_and_shutdown_drains_writer() {
    let directory = std::env::temp_dir().join(format!("argusflow-settle-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(directory.join("evidence")).unwrap();
    let desktop = Arc::new(DelayedDesktop(Instant::now()));
    let collector = Arc::new(EvidenceCollector::new(
        desktop.clone(),
        desktop.clone(),
        desktop.clone(),
        desktop,
    ));
    let writer = crate::screenshot_pipeline::ScreenshotWriter::start(directory.clone()).unwrap();
    let capture =
        crate::post_capture::PostCapture::start(collector, writer, Instant::now(), true).unwrap();
    let result = capture
        .submit(
            PhysicalEvent {
                sequence: 1,
                timestamp_ms: 0,
                input: PhysicalInput::Key {
                    virtual_key: 0x5B,
                    scan_code: 0,
                    flags: 0,
                    phase: InputPhase::Down,
                },
            },
            10,
            None,
        )
        .unwrap();
    // Drop 必须排空尚未到期的采样和 PNG，再允许发布 manifest。
    tokio::task::spawn_blocking(move || drop(capture))
        .await
        .unwrap();
    let evidence = result.await.unwrap().unwrap();
    assert!(evidence.captured_at_ms >= 900);
    assert!(evidence.stabilized);
    let mut decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(directory.join(&evidence.path)).unwrap(),
    ))
    .read_info()
    .unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.next_frame(&mut pixels).unwrap();
    assert_eq!(pixels, vec![255; 64]);
    drop(decoder);
    std::fs::remove_dir_all(directory).unwrap();
}
