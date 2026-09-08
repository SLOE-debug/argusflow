//! 真实桌面的反查与控制面测试。必须显式 --ignored，不操作用户已有应用。

#[path = "support/native_window.rs"]
mod native_window;
#[path = "support/physical_assertions.rs"]
mod physical_assertions;

use argusflow_core::*;
use argusflow_recorder::*;
use argusflow_windows::{
    capture::WindowsEventCapture, uia::UiaRuntime, window::WindowsWindowInspector,
};
use async_trait::async_trait;
use std::{sync::Arc, time::Duration};

/// Fixture 不附加浏览器或启动 OCR；验证真实 Windows/UIA 后端分支。
struct Unavailable;
#[async_trait]
impl TargetInspector for Unavailable {
    async fn inspect(
        &self,
        _: &InspectionContext,
        _: InspectionProbe,
    ) -> Result<InspectedEntity, InspectionFailure> {
        Err(InspectionFailure::Unavailable)
    }
}

/// 测试只允许解析 fixture 窗口，用户误切其他应用时输入按未知字段遮盖。
struct FixtureWindowInspector(u64);
impl WindowInspector for FixtureWindowInspector {
    fn window_context(
        &self,
        window: WindowIdentity,
    ) -> Result<InspectionContext, InspectionFailure> {
        if window.handle != self.0 {
            return Err(InspectionFailure::ContextChanged);
        }
        WindowsWindowInspector.window_context(window)
    }
    fn context(&self, probe: InspectionProbe) -> Result<InspectionContext, InspectionFailure> {
        let context = WindowsWindowInspector.context(probe)?;
        if context.window.handle != self.0 {
            return Err(InspectionFailure::ContextChanged);
        }
        Ok(context)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires physical clicks and fixed text in the temporary Win32 fixture within 90 seconds"]
async fn physical_input_to_event_timeline_and_evidence() {
    let fixture = native_window::NativeWindow::start();
    let uia = Arc::new(UiaRuntime::start());
    for _ in 0..50 {
        if uia.health().is_ready() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let resolver = Arc::new(EvidenceCollector::new(
        Arc::new(FixtureWindowInspector(fixture.handle)),
        Arc::new(Unavailable),
        uia,
        Arc::new(WindowsEventCapture::default()),
    ));
    let root = std::env::temp_dir().join(format!("argusflow-physical-{}", uuid::Uuid::new_v4()));
    let recorder = RecorderService::new(resolver, &root);
    recorder
        .start_with_privacy(RecordingPrivacy {
            enabled: true,
            sensitive_input: true,
            ..Default::default()
        })
        .await
        .unwrap();
    println!(
        "PHYSICAL TIMELINE READY: use English layout; click top field, type argus; click bottom, type secret42; press Enter. Automatic stop after 90 seconds."
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    let mut finished = false;
    while tokio::time::Instant::now() < deadline {
        if fixture.finished() {
            finished = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // Enter 的真实 key-up 需要经过 Hook、摄入线程与有序 worker；stop 会排空整个链。
    let completed = recorder.stop().await.unwrap();
    let loaded = recorder.load(completed.trace.recording_id).await.unwrap();
    assert_eq!(loaded.trace.timeline, completed.trace.timeline);
    // 在断言之前清理固定测试目录，失败也不会留下录制内容；只删除显式返回的文件。
    cleanup(&root).await;
    assert!(
        finished,
        "no completion Enter in fixture before deadline; Hook has been stopped"
    );
    physical_assertions::assert_physical_trace(&completed.trace, fixture.handle);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "shows a temporary native Win32 window and installs real global hooks"]
async fn real_uia_hit_test_password_and_hook_service_save_retry() {
    let fixture = native_window::NativeWindow::start();
    let windows = WindowsWindowInspector;
    let uia = Arc::new(UiaRuntime::start());
    // UIA MTA 初始化异步完成，等待其真实健康状态。
    for _ in 0..50 {
        if uia.health().is_ready() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let resolver = Arc::new(EvidenceCollector::new(
        Arc::new(windows),
        Arc::new(Unavailable),
        uia,
        Arc::new(WindowsEventCapture::default()),
    ));
    for (point, sensitivity) in [
        (fixture.ordinary, FieldSensitivity::Normal),
        (fixture.password, FieldSensitivity::Sensitive),
    ] {
        let probe = InspectionProbe::Point(point);
        let context = WindowsWindowInspector.context(probe).unwrap();
        assert_eq!(context.window.handle, fixture.handle);
        assert_eq!(context.window.process_id, std::process::id());
        let target = resolver.collect(context, probe, 0, 0).await;
        assert_eq!(
            target.ui_snapshot.as_ref().unwrap().backend,
            EvidenceBackend::Uia,
            "diagnostics={:?}",
            target.diagnostics
        );
        let entity = &target.ui_snapshot.as_ref().unwrap().entity;
        assert_eq!(entity.semantics.role, Some(ElementRole::TextBox));
        assert_eq!(entity.sensitivity, sensitivity);
        assert_eq!(
            entity.semantics.automation_id.as_deref(),
            Some(if sensitivity == FieldSensitivity::Normal {
                "1001"
            } else {
                "1002"
            })
        );
        assert_eq!(
            entity.bounds.width, 490.0,
            "UIA cache bounds must remain screen physical pixels"
        );
        assert!(entity.bounds.contains(point));
        // Provider 只报告敏感性；遮盖由录制会话显式配置。
        println!("real WindowFromPoint/UIA evidence: TextBox, sensitivity={sensitivity:?}");
    }
    fixture.focus_password();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let context = WindowsWindowInspector
        .context(InspectionProbe::Focus)
        .unwrap();
    assert_eq!(context.window.handle, fixture.handle);
    let focus = resolver
        .collect(context, InspectionProbe::Focus, 0, 0)
        .await;
    assert_eq!(
        focus.ui_snapshot.unwrap().entity.sensitivity,
        FieldSensitivity::Sensitive
    );

    let root = std::env::temp_dir().join(format!("argusflow-live-{}", uuid::Uuid::new_v4()));
    // 根路径先放普通文件，确定性制造保存失败，避免修改用户目录权限。
    tokio::fs::write(&root, b"test blocker").await.unwrap();
    let recorder = RecorderService::new(resolver, &root);
    assert_eq!(recorder.status().await.phase, RecorderPhase::Idle);
    // 截图目录必须在监听前可写，开始失败不能留下 Hook。
    assert!(matches!(
        recorder.start().await,
        Err(RecorderError::Storage(_))
    ));
    tokio::fs::remove_file(&root).await.unwrap();
    assert_eq!(
        recorder.start().await.unwrap().phase,
        RecorderPhase::Recording
    );
    assert!(matches!(
        recorder.start().await,
        Err(RecorderError::AlreadyRecording)
    ));
    let recording_id = recorder.status().await.recording_id.unwrap();
    // 用同名目录阻止 timeline rename，不影响已经冻结的截图。
    let blocker = root.join(recording_id.to_string()).join("timeline.json");
    tokio::fs::create_dir(&blocker).await.unwrap();
    assert!(matches!(
        recorder.stop().await,
        Err(RecorderError::Storage(_))
    ));
    assert_eq!(recorder.status().await.phase, RecorderPhase::AwaitingSave);
    tokio::fs::remove_dir(blocker).await.unwrap();
    let completed = recorder.stop().await.unwrap();
    assert_eq!(recorder.status().await.phase, RecorderPhase::Idle);
    let saved = recorder.load(completed.trace.recording_id).await.unwrap();
    assert_eq!(saved.trace.timeline, completed.trace.timeline);
    assert_eq!(recorder.list().await.unwrap().len(), 1);
    assert!(matches!(
        recorder.stop().await,
        Err(RecorderError::NotRecording)
    ));
    cleanup(&root).await;
    println!(
        "real Hook service: start, duplicate rejection, stop, save failure, retry, history roundtrip passed"
    );
}

/// 清理仅由本测试生成的临时 UUID 目录，先验证绝对目标归属。
async fn cleanup(root: &std::path::Path) {
    let directory = tokio::fs::canonicalize(root).await.unwrap();
    let temporary = tokio::fs::canonicalize(std::env::temp_dir()).await.unwrap();
    assert_eq!(directory.parent(), Some(temporary.as_path()));
    assert!(
        directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("argusflow-")
    );
    tokio::fs::remove_dir_all(directory).await.unwrap();
}
