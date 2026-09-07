//! 真实桌面的反查与控制面测试。必须显式 --ignored，不操作用户已有应用。

#[path = "support/native_window.rs"]
mod native_window;
#[path = "support/physical_assertions.rs"]
mod physical_assertions;

use argusflow_core::*;
use argusflow_recorder::*;
use argusflow_windows::{uia::UiaRuntime, window::WindowsWindowInspector};
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
async fn physical_input_to_semantic_trace_and_redaction() {
    let fixture = native_window::NativeWindow::start();
    let uia = Arc::new(UiaRuntime::start());
    for _ in 0..50 {
        if uia.health().is_ready() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let resolver = Arc::new(TargetResolver::new(
        Arc::new(FixtureWindowInspector(fixture.handle)),
        Arc::new(Unavailable),
        uia,
        Arc::new(Unavailable),
    ));
    let root = std::env::temp_dir().join(format!("argusflow-physical-{}", uuid::Uuid::new_v4()));
    let recorder = RecorderService::new(resolver, &root);
    recorder.start().await.unwrap();
    println!(
        "PHYSICAL SEMANTIC READY: use English layout; click top field, type argus; click bottom, type secret42; press Enter. Automatic stop after 90 seconds."
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
    assert_eq!(loaded.trace.raw, completed.trace.raw);
    assert_eq!(loaded.trace.normalized, completed.trace.normalized);
    // 在断言之前清理固定测试目录，失败也不会留下录制内容；只删除显式返回的文件。
    for file in [
        &completed.files.raw,
        &completed.files.normalized,
        &completed.files.manifest,
    ] {
        tokio::fs::remove_file(file).await.unwrap();
    }
    tokio::fs::remove_dir(root.join(completed.trace.recording_id.to_string()))
        .await
        .unwrap();
    tokio::fs::remove_dir(root).await.unwrap();
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
    let resolver = Arc::new(TargetResolver::new(
        Arc::new(windows),
        Arc::new(Unavailable),
        uia,
        Arc::new(Unavailable),
    ));
    for (point, sensitivity) in [
        (fixture.ordinary, FieldSensitivity::Normal),
        (fixture.password, FieldSensitivity::Sensitive),
    ] {
        let probe = InspectionProbe::Point(point);
        let context = WindowsWindowInspector.context(probe).unwrap();
        assert_eq!(context.window.handle, fixture.handle);
        assert_eq!(context.window.process_id, std::process::id());
        let target = resolver.resolve(context, probe).await;
        assert_eq!(
            target.backend,
            ResolutionBackend::Uia,
            "diagnostics={:?}",
            target.diagnostics
        );
        let entity = target.entity.as_ref().unwrap();
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
        assert!(!target.selector_candidates.is_empty());
        if sensitivity == FieldSensitivity::Sensitive {
            assert!(entity.semantics.name.is_none());
        }
        println!(
            "real WindowFromPoint/UIA: TextBox, sensitivity={sensitivity:?}, candidates={}",
            target.selector_candidates.len()
        );
    }
    fixture.focus_password();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let context = WindowsWindowInspector
        .context(InspectionProbe::Focus)
        .unwrap();
    assert_eq!(context.window.handle, fixture.handle);
    let focus = resolver.resolve(context, InspectionProbe::Focus).await;
    assert_eq!(
        focus.entity.unwrap().sensitivity,
        FieldSensitivity::Sensitive
    );

    let root = std::env::temp_dir().join(format!("argusflow-live-{}", uuid::Uuid::new_v4()));
    // 根路径先放普通文件，确定性制造保存失败，避免修改用户目录权限。
    tokio::fs::write(&root, b"test blocker").await.unwrap();
    let recorder = RecorderService::new(resolver, &root);
    assert_eq!(recorder.status().await.phase, RecorderPhase::Idle);
    assert_eq!(
        recorder.start().await.unwrap().phase,
        RecorderPhase::Recording
    );
    assert!(matches!(
        recorder.start().await,
        Err(RecorderError::AlreadyRecording)
    ));
    assert!(matches!(
        recorder.stop().await,
        Err(RecorderError::Storage(_))
    ));
    assert_eq!(recorder.status().await.phase, RecorderPhase::AwaitingSave);
    tokio::fs::remove_file(&root).await.unwrap();
    let completed = recorder.stop().await.unwrap();
    assert_eq!(recorder.status().await.phase, RecorderPhase::Idle);
    let saved = recorder.load(completed.trace.recording_id).await.unwrap();
    assert_eq!(saved.trace.normalized, completed.trace.normalized);
    assert_eq!(recorder.list().await.unwrap().len(), 1);
    assert!(matches!(
        recorder.stop().await,
        Err(RecorderError::NotRecording)
    ));
    // 仅删除本次 UUID fixture 返回的三个文件及其空目录。
    for file in [
        &completed.files.raw,
        &completed.files.normalized,
        &completed.files.manifest,
    ] {
        tokio::fs::remove_file(file).await.unwrap();
    }
    tokio::fs::remove_dir(root.join(completed.trace.recording_id.to_string()))
        .await
        .unwrap();
    tokio::fs::remove_dir(root).await.unwrap();
    println!(
        "real Hook service: start, duplicate rejection, stop, save failure, retry, history roundtrip passed"
    );
}
