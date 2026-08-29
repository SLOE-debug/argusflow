//! 视觉场景刷新任务的端到端截止时间、阶段追踪与 panic 隔离。

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use argusflow_core::WindowIdentity;

use crate::{
    diagnostics::persist_scene_timeout,
    error::{SceneExecutionPhase, VisionError},
    ocr::OcrRequest,
    runtime::{SceneRefreshPolicy, VisionRuntime},
    scene::VisualScene,
};

/// 给捕获订阅建立、任务调度和 Named Pipe 传输保留的固定预算。
const SCENE_EXECUTION_GRACE: Duration = Duration::from_secs(2);

/// 场景执行任务在同步平台调用之外维护的最小诊断状态。
#[derive(Debug)]
pub(crate) struct SceneExecutionTrace {
    /// 最后进入的阶段与最近形成的 OCR 请求必须成对快照，避免诊断自相矛盾。
    state: Mutex<SceneExecutionState>,
}

/// 一次端到端场景刷新可供超时处理器读取的状态。
#[derive(Debug)]
struct SceneExecutionState {
    /// 任务最后进入的执行阶段。
    phase: SceneExecutionPhase,
    /// 最近形成的拥有型 OCR 输入；正常完成时不会持久化。
    ocr_request: Option<OcrRequest>,
}

impl SceneExecutionTrace {
    /// 从缓存查询阶段创建一份空执行轨迹。
    fn new() -> Self {
        Self {
            state: Mutex::new(SceneExecutionState {
                phase: SceneExecutionPhase::CacheLookup,
                ocr_request: None,
            }),
        }
    }

    /// 在进入可能失败或阻塞的阶段前更新轨迹。
    pub(crate) fn enter(&self, phase: SceneExecutionPhase) {
        self.lock_state().phase = phase;
    }

    /// 只在内存中保留最近形成的 OCR 输入，直到确认需要输出失败现场。
    pub(crate) fn record_ocr_input(&self, request: &OcrRequest) {
        self.lock_state().ocr_request = Some(request.clone());
    }

    /// 取得相互一致的阶段与 OCR 输入快照。
    fn snapshot(&self) -> SceneExecutionState {
        let state = self.lock_state();
        SceneExecutionState {
            phase: state.phase,
            ocr_request: state.ocr_request.clone(),
        }
    }

    /// 即使任务 panic 造成互斥锁中毒，也保留最后可用的诊断状态。
    fn lock_state(&self) -> std::sync::MutexGuard<'_, SceneExecutionState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

/// 在独立 Tokio 任务中刷新场景，使同步平台调用无法阻塞父任务的截止计时器。
pub(crate) async fn current_scene_with_deadline(
    runtime: Arc<VisionRuntime>,
    window: WindowIdentity,
    policy: SceneRefreshPolicy,
) -> Result<Arc<VisualScene>, VisionError> {
    let timeout = policy
        .stability
        .timeout
        .saturating_add(policy.ocr_timeout)
        .saturating_add(SCENE_EXECUTION_GRACE);
    let trace = Arc::new(SceneExecutionTrace::new());
    let task_trace = trace.clone();
    let mut task = tokio::spawn(async move {
        runtime
            .current_scene_traced(window, &policy, task_trace.as_ref())
            .await
    });
    match tokio::time::timeout(timeout, &mut task).await {
        Ok(Ok(Err(VisionError::FrameTimeout { timeout_ms }))) => {
            Err(scene_timeout_error(trace.as_ref(), timeout_ms))
        }
        Ok(Ok(Err(VisionError::FrameUnstable { timeout_ms, .. }))) => {
            Err(scene_timeout_error(trace.as_ref(), timeout_ms))
        }
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(VisionError::OcrFailed {
            message: format!("visual scene task terminated unexpectedly: {error}"),
        }),
        Err(_) => {
            task.abort();
            let timeout_ms = timeout.as_millis().min(u128::from(u64::MAX)) as u64;
            Err(scene_timeout_error(trace.as_ref(), timeout_ms))
        }
    }
}

/// 将管线内部或端到端超时统一补充为带阶段和失败现场的错误。
fn scene_timeout_error(trace: &SceneExecutionTrace, timeout_ms: u64) -> VisionError {
    let snapshot = trace.snapshot();
    let diagnostic =
        persist_scene_timeout(snapshot.phase, timeout_ms, snapshot.ocr_request.as_ref());
    VisionError::SceneTimeout {
        timeout_ms,
        phase: snapshot.phase,
        diagnostic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_reports_the_last_entered_phase_without_an_ocr_input() {
        let trace = SceneExecutionTrace::new();
        trace.enter(SceneExecutionPhase::WaitingForStableFrame);

        let snapshot = trace.snapshot();

        assert_eq!(snapshot.phase, SceneExecutionPhase::WaitingForStableFrame);
        assert!(snapshot.ocr_request.is_none());
    }
}
