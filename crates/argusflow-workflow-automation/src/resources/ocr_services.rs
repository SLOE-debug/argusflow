//! 共享服务的配置一致性、并发初始化与宿主关闭状态。
use super::ocr_session::Session;
use argusflow_capture_contracts::FrameConfig;
use argusflow_core::Operation;
use argusflow_runtime::RunError;
use argusflow_workflow::ErrorKind;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Mutex;
/// 宿主共享的 OCR 服务管理器，首次请求时加载，显式关闭后不再接受请求。
#[derive(Clone, Default)]
pub struct OcrServices {
    state: Arc<Mutex<State>>,
    capture: FrameConfig,
    dependencies: Option<PathBuf>,
}
#[derive(Default)]
struct State {
    active: Option<Arc<Session>>,
    closed: bool,
}
impl OcrServices {
    /// 宿主设置采集分辨率和内存预算，节点不能各自启动不同服务。
    pub fn new(dependencies: PathBuf, capture: FrameConfig) -> Self {
        Self {
            state: Arc::default(),
            capture,
            dependencies: Some(dependencies),
        }
    }
    pub(crate) async fn acquire(&self, operation: &Operation) -> Result<Arc<Session>, RunError> {
        let mut state = self.lock(operation).await?;
        operation.check("ocr_services_acquire")?;
        if state.closed {
            return Err(RunError::new(ErrorKind::Unavailable, "OCR 宿主服务已关闭"));
        }
        if let Some(session) = &state.active {
            return Ok(session.clone());
        }
        let dependencies = self
            .dependencies
            .as_ref()
            .ok_or_else(|| RunError::new(ErrorKind::Unavailable, "宿主未装配内置 OCR 资源"))?;
        let session =
            Arc::new(Session::start(dependencies, self.capture.clone(), operation).await?);
        state.active = Some(session.clone());
        Ok(session)
    }
    /// 所有工作流收尾后由宿主调用；关闭后拒绝新绑定。
    pub async fn shutdown(&self, operation: &Operation) -> Result<(), RunError> {
        let mut state = self.lock(operation).await?;
        state.closed = true;
        if let Some(session) = &state.active {
            session.shutdown(operation).await?;
        }
        state.active = None;
        Ok(())
    }
    async fn lock(
        &self,
        operation: &Operation,
    ) -> Result<tokio::sync::MutexGuard<'_, State>, RunError> {
        loop {
            operation.check("ocr_services_lock")?;
            tokio::select! {
                state = self.state.lock() => return Ok(state),
                _ = tokio::time::sleep(Duration::from_millis(20).min(operation.remaining())) => {}
            }
        }
    }
}
#[cfg(test)]
#[path = "../../../../tests/argusflow-workflow-automation/unit/ocr_services.rs"]
mod tests;
