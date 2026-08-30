//! 桌面首屏读取的强类型能力启动快照与事件发布循环。

use std::time::Duration;

use argusflow_vision::{
    CaptureLifecycle, VisionHealth, WorkerDevice, WorkerLifecycle, WorkerModelInfo,
    WorkerModelLifecycle,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::runtime::AppState;

/// 前端订阅的启动快照事件名。
pub const STARTUP_STATUS_EVENT: &str = "argusflow://startup-status";

/// 启动门控结论；WGC 与两个 OCR 档位全部就绪后才进入 Home。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupReadiness {
    /// WGC 或任一 OCR 档位仍在初始化。
    Loading,
    /// WGC、Small OCR 与 Medium OCR 均已就绪。
    Ready,
    /// 任一能力确定失败，用户可以重试或进入降级工作台。
    Blocked,
}

/// 当前最值得向用户说明的启动阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupPhase {
    /// 应用运行时刚完成轻量装配。
    StartingRuntime,
    /// WGC 捕获线程或 D3D11 设备正在初始化。
    InitializingCapture,
    /// OCR worker 正在探测 CUDA。
    SelectingOcrDevice,
    /// Small 模型正在加载权重。
    LoadingSmallModel,
    /// Small 模型正在执行真实文本预热。
    WarmingSmallModel,
    /// Medium 模型正在加载权重。
    LoadingMediumModel,
    /// Medium 模型正在执行真实文本预热。
    WarmingMediumModel,
    /// 全部本地能力可用。
    Ready,
    /// 任一能力确定失败。
    Failed,
}

/// 单项启动能力的可展示状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupComponentLifecycle {
    /// 尚未开始或还没有收到 worker health。
    Pending,
    /// 正在创建资源或加载权重。
    Initializing,
    /// 正在执行首次完整推理。
    Warming,
    /// 可以接受真实请求。
    Ready,
    /// 初始化失败。
    Failed,
}

/// 一项启动能力的状态及安全错误摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupComponentStatus {
    /// 当前生命周期。
    pub lifecycle: StartupComponentLifecycle,
    /// 失败或降级时不含捕获文本的说明。
    pub message: Option<String>,
}

/// 前端启动页与状态栏共享的完整快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupSnapshot {
    /// 是否允许自动进入工作台。
    pub readiness: StartupReadiness,
    /// 当前主要阶段。
    pub phase: StartupPhase,
    /// 已完成的 WGC、Small、Medium 步骤数。
    pub completed_steps: u8,
    /// 固定步骤总数。
    pub total_steps: u8,
    /// WGC 状态。
    pub capture: StartupComponentStatus,
    /// Small OCR 状态。
    pub small_ocr: StartupComponentStatus,
    /// Medium OCR 状态，参与进入 Home 的门控。
    pub medium_ocr: StartupComponentStatus,
    /// worker 实际选择的设备。
    pub device: Option<WorkerDevice>,
    /// GPU 转 CPU 或 Medium 失败等可继续工作的原因。
    pub degradation_reason: Option<String>,
}

impl StartupSnapshot {
    /// 从统一视觉健康状态构造前端契约。
    pub fn from_health(health: VisionHealth, startup_elapsed: Duration) -> Self {
        let capture = capture_status(&health);
        let worker_failure_is_final = startup_elapsed >= Duration::from_secs(30)
            || health
                .worker
                .degradation_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("not configured"));
        let small_model = model(&health, argusflow_vision::OcrModel::PpOcrV6Small);
        let medium_model = model(&health, argusflow_vision::OcrModel::PpOcrV6Medium);
        let small_ocr = model_status(&health, small_model, worker_failure_is_final);
        let medium_ocr = model_status(&health, medium_model, worker_failure_is_final);
        let capture_ready = capture.lifecycle == StartupComponentLifecycle::Ready;
        let small_ready = small_ocr.lifecycle == StartupComponentLifecycle::Ready;
        let medium_ready = medium_ocr.lifecycle == StartupComponentLifecycle::Ready;
        let initialization_failed = capture.lifecycle == StartupComponentLifecycle::Failed
            || small_ocr.lifecycle == StartupComponentLifecycle::Failed
            || medium_ocr.lifecycle == StartupComponentLifecycle::Failed;
        let readiness = if capture_ready && small_ready && medium_ready {
            StartupReadiness::Ready
        } else if initialization_failed {
            StartupReadiness::Blocked
        } else {
            StartupReadiness::Loading
        };
        let phase = startup_phase(&health, &capture, &small_ocr, &medium_ocr, readiness);
        let completed_steps = [&capture, &small_ocr, &medium_ocr]
            .into_iter()
            .filter(|status| status.lifecycle == StartupComponentLifecycle::Ready)
            .count() as u8;
        let device = small_model.or(medium_model).map(|model| model.device);
        Self {
            readiness,
            phase,
            completed_steps,
            total_steps: 3,
            capture,
            small_ocr,
            medium_ocr,
            device,
            degradation_reason: health.worker.degradation_reason,
        }
    }
}

/// React 首屏触发初始化后启动 health 轮询，并把快照推送给已经挂载的启动页。
pub fn start_status_publisher(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app_handle.state::<AppState>();
            state.refresh_worker_health().await;
            let snapshot =
                StartupSnapshot::from_health(state.vision_health(), state.startup_elapsed());
            let _ = app_handle.emit(STARTUP_STATUS_EVENT, snapshot.clone());
            let interval = if snapshot.readiness == StartupReadiness::Loading {
                Duration::from_millis(250)
            } else {
                Duration::from_secs(2)
            };
            tokio::time::sleep(interval).await;
        }
    });
}

/// 把跨平台捕获生命周期映射为 Tauri 稳定契约。
fn capture_status(health: &VisionHealth) -> StartupComponentStatus {
    let lifecycle = match health.capture.lifecycle {
        CaptureLifecycle::Pending => StartupComponentLifecycle::Pending,
        CaptureLifecycle::Initializing => StartupComponentLifecycle::Initializing,
        CaptureLifecycle::Ready => StartupComponentLifecycle::Ready,
        CaptureLifecycle::Failed => StartupComponentLifecycle::Failed,
    };
    StartupComponentStatus {
        lifecycle,
        message: health.capture.message.clone(),
    }
}

/// 查找指定强类型模型档位的状态。
fn model(health: &VisionHealth, target: argusflow_vision::OcrModel) -> Option<&WorkerModelInfo> {
    health
        .worker
        .models
        .iter()
        .find(|model| model.model == target)
}

/// 把模型或连接阶段映射为前端组件状态。
fn model_status(
    health: &VisionHealth,
    model: Option<&WorkerModelInfo>,
    worker_failure_is_final: bool,
) -> StartupComponentStatus {
    if let Some(model) = model {
        let lifecycle = match model.lifecycle {
            WorkerModelLifecycle::Pending | WorkerModelLifecycle::Loading => {
                StartupComponentLifecycle::Initializing
            }
            WorkerModelLifecycle::Warming => StartupComponentLifecycle::Warming,
            WorkerModelLifecycle::Ready => StartupComponentLifecycle::Ready,
            WorkerModelLifecycle::Failed => StartupComponentLifecycle::Failed,
        };
        return StartupComponentStatus {
            lifecycle,
            message: model.message.clone(),
        };
    }
    let lifecycle = match health.worker.lifecycle {
        WorkerLifecycle::Starting => StartupComponentLifecycle::Pending,
        WorkerLifecycle::SelectingDevice | WorkerLifecycle::LoadingModels => {
            StartupComponentLifecycle::Initializing
        }
        WorkerLifecycle::Ready | WorkerLifecycle::Degraded => StartupComponentLifecycle::Pending,
        WorkerLifecycle::Failed if worker_failure_is_final => StartupComponentLifecycle::Failed,
        WorkerLifecycle::Failed => StartupComponentLifecycle::Initializing,
    };
    StartupComponentStatus {
        lifecycle,
        message: (lifecycle == StartupComponentLifecycle::Failed)
            .then(|| health.worker.degradation_reason.clone())
            .flatten(),
    }
}

/// 选择比百分比更有行动意义的当前阶段。
fn startup_phase(
    health: &VisionHealth,
    capture: &StartupComponentStatus,
    small: &StartupComponentStatus,
    medium: &StartupComponentStatus,
    readiness: StartupReadiness,
) -> StartupPhase {
    if readiness == StartupReadiness::Blocked {
        return StartupPhase::Failed;
    }
    if readiness == StartupReadiness::Ready {
        return StartupPhase::Ready;
    }
    if capture.lifecycle == StartupComponentLifecycle::Pending {
        return StartupPhase::StartingRuntime;
    }
    if capture.lifecycle == StartupComponentLifecycle::Initializing {
        return StartupPhase::InitializingCapture;
    }
    if health.worker.lifecycle == WorkerLifecycle::SelectingDevice {
        return StartupPhase::SelectingOcrDevice;
    }
    if small.lifecycle == StartupComponentLifecycle::Warming {
        return StartupPhase::WarmingSmallModel;
    }
    if small.lifecycle != StartupComponentLifecycle::Ready {
        return StartupPhase::LoadingSmallModel;
    }
    if medium.lifecycle == StartupComponentLifecycle::Warming {
        return StartupPhase::WarmingMediumModel;
    }
    StartupPhase::LoadingMediumModel
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use argusflow_vision::{
        CaptureHealth, CaptureLifecycle, OcrModel, VISION_PROTOCOL_VERSION, VisionHealth,
        WorkerDevice, WorkerHealth, WorkerInferenceEngine, WorkerLifecycle, WorkerModelInfo,
        WorkerModelLifecycle,
    };

    use super::{StartupPhase, StartupReadiness, StartupSnapshot};

    /// 创建不依赖真实 Paddle 或 WGC 的健康状态。
    fn health(worker_lifecycle: WorkerLifecycle, models: Vec<WorkerModelInfo>) -> VisionHealth {
        let worker = WorkerHealth {
            protocol_version: VISION_PROTOCOL_VERSION.to_owned(),
            worker_version: "test-worker".to_owned(),
            paddleocr_version: "3.7.0".to_owned(),
            lifecycle: worker_lifecycle,
            models,
            queue_depth: 0,
            degradation_reason: None,
        };
        VisionHealth {
            capture: CaptureHealth::new(CaptureLifecycle::Ready),
            worker_ready: worker.is_ready(),
            worker,
        }
    }

    /// 创建一个指定档位和生命周期的模型状态。
    fn model(model: OcrModel, lifecycle: WorkerModelLifecycle) -> WorkerModelInfo {
        WorkerModelInfo {
            model,
            device: WorkerDevice::Cuda { index: 0 },
            engine: WorkerInferenceEngine::PaddleStatic,
            lifecycle,
            message: None,
        }
    }

    #[test]
    fn medium_loading_keeps_home_entry_gated() {
        let snapshot = StartupSnapshot::from_health(
            health(
                WorkerLifecycle::Ready,
                vec![
                    model(OcrModel::PpOcrV6Small, WorkerModelLifecycle::Ready),
                    model(OcrModel::PpOcrV6Medium, WorkerModelLifecycle::Loading),
                ],
            ),
            Duration::from_secs(2),
        );

        assert_eq!(snapshot.readiness, StartupReadiness::Loading);
        assert_eq!(snapshot.phase, StartupPhase::LoadingMediumModel);
        assert_eq!(snapshot.completed_steps, 2);
        assert_eq!(snapshot.device, Some(WorkerDevice::Cuda { index: 0 }));
    }

    #[test]
    fn all_capabilities_ready_allow_home_entry() {
        let snapshot = StartupSnapshot::from_health(
            health(
                WorkerLifecycle::Ready,
                vec![
                    model(OcrModel::PpOcrV6Small, WorkerModelLifecycle::Ready),
                    model(OcrModel::PpOcrV6Medium, WorkerModelLifecycle::Ready),
                ],
            ),
            Duration::from_secs(2),
        );

        assert_eq!(snapshot.readiness, StartupReadiness::Ready);
        assert_eq!(snapshot.phase, StartupPhase::Ready);
        assert_eq!(snapshot.completed_steps, 3);
    }

    #[test]
    fn worker_connection_failure_gets_a_startup_grace_period() {
        let loading = StartupSnapshot::from_health(
            health(WorkerLifecycle::Failed, Vec::new()),
            Duration::from_secs(5),
        );
        let blocked = StartupSnapshot::from_health(
            health(WorkerLifecycle::Failed, Vec::new()),
            Duration::from_secs(31),
        );

        assert_eq!(loading.readiness, StartupReadiness::Loading);
        assert_eq!(blocked.readiness, StartupReadiness::Blocked);
    }
}
