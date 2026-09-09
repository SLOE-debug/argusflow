//! 原生持续采集及重启回归，不安装输入 Hook；临时截图在断言前删除。
use super::*;
use argusflow_core::*;

#[path = "../../argusflow-windows/examples/support/capture_fixture.rs"]
#[allow(dead_code)] // 复用原生窗口 fixture，启动测试不调用完整像素验证接口。
mod fixture;

struct Unavailable;
#[async_trait::async_trait]
impl TargetInspector for Unavailable {
    async fn inspect(
        &self,
        _: &InspectionContext,
        _: InspectionProbe,
    ) -> Result<InspectedEntity, InspectionFailure> {
        Err(InspectionFailure::Unavailable)
    }
}

#[test]
#[ignore = "captures the real desktop briefly without installing input hooks"]
fn native_archive_sustained_capture_and_restart() {
    let painting = Arc::new(AtomicBool::new(true));
    let painting_thread = painting.clone();
    let painter = std::thread::spawn(move || {
        let fixture = fixture::CaptureFixture::new(800, 600).unwrap();
        let mut step = 0;
        while painting_thread.load(Ordering::Acquire) {
            fixture.paint(step).unwrap();
            step = (step + 1) % 100;
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    let collector = Arc::new(EvidenceCollector::new(
        Arc::new(argusflow_windows::window::WindowsWindowInspector),
        Arc::new(Unavailable),
        Arc::new(Unavailable),
        Arc::new(argusflow_windows::capture::WindowsEventCapture::default()),
    ));
    let directory =
        std::env::temp_dir().join(format!("argusflow-startup-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    // 不存在的归档目录必须保留 Storage 分类，失败不能污染共享主机的下次订阅。
    let failed = ScreenRecording::start(
        collector.clone(),
        directory.join("missing"),
        collector.screen_clock_us().unwrap(),
    );
    assert!(matches!(
        failed,
        Err(RecorderError::CaptureStartup {
            reason: CaptureFailure::Storage,
            ..
        })
    ));
    // 快速启停无法覆盖差分堆积；每次必须持续跨越两个 2 秒检查点。
    let result = (0..3)
        .map(|attempt| {
            let attempt_directory = directory.join(attempt.to_string());
            std::fs::create_dir(&attempt_directory).unwrap();
            let mut recording = ScreenRecording::start(
                collector.clone(),
                attempt_directory.clone(),
                collector.screen_clock_us().unwrap(),
            )?;
            std::thread::sleep(Duration::from_secs(5));
            let timeline = recording.finish();
            println!(
                "frames={} last_us={:?} completeness={:?} diagnostics={:?}",
                timeline.frames.len(),
                timeline.frames.last().map(|frame| frame.presented_us),
                timeline.completeness,
                timeline.diagnostics
            );
            let mut differ = collector.pixel_differ().map_err(|error|RecorderError::Refinement(error.to_string()))?;
            let refined = crate::screen_refinement::refine(&attempt_directory, &timeline, differ.as_mut())?;
            println!("precise_frames={} duration_us={} refinement={:?}", refined.timeline.frames.len(), refined.timeline.duration_us, refined.timeline.refinement);
            Ok::<_, RecorderError>(refined.timeline.clone())
        })
        .collect::<Result<Vec<_>, _>>();
    collector.shutdown_capture().unwrap();
    painting.store(false, Ordering::Release);
    painter.join().unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    for timeline in result.expect("native archive must establish a baseline on every restart") {
        assert!(!timeline.frames.is_empty());
        assert_eq!(timeline.completeness, ScreenCompleteness::Complete);
        assert_eq!(timeline.refinement, ScreenRefinement::Complete);
        assert!(timeline.frames.last().unwrap().presented_us >= 4_900_000);
    }
}
