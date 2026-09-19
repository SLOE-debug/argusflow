//! 窗口 OCR 动态范围；不持有模型或采集线程的关闭权。
use super::QuerySourceProvider;
use argusflow_automation::QuerySource;
use argusflow_capture_contracts::{PixelRect, SourceState};
use argusflow_core::{Failure, Operation};
use argusflow_runtime::{RunError, TaskFuture};
use argusflow_windows::{InputService, WindowIdentity};
use argusflow_workflow::ErrorKind;
use std::{sync::Arc, time::Duration};
pub(crate) struct OcrWindow {
    pub session: Arc<super::ocr_session::Session>,
    pub window: WindowIdentity,
    pub input: InputService,
}
impl OcrWindow {
    pub async fn ready(&self, operation: &Operation) -> Result<(), RunError> {
        self.input
            .perform_operation(
                self.window.clone(),
                argusflow_windows::InputAction::ActivateWindow,
                operation,
            )
            .await
            .map_err(Failure::from)?;
        loop {
            operation.check("workflow_ocr_ready")?;
            let sources = self
                .session
                .frames
                .sources()
                .map_err(|e| RunError::new(ErrorKind::Unavailable, e.to_string()))?;
            if sources.iter().any(|s| s.state == SourceState::Ready) {
                return self.source().map(|_| ());
            }
            tokio::time::sleep(Duration::from_millis(20).min(operation.remaining())).await;
        }
    }
    pub fn source(&self) -> Result<QuerySource, RunError> {
        self.window.require_foreground().map_err(Failure::from)?;
        let bounds = self.window.interaction_bounds().map_err(Failure::from)?;
        let sources = self
            .session
            .frames
            .sources()
            .map_err(|e| RunError::new(ErrorKind::Unavailable, e.to_string()))?;
        let mut found = Vec::new();
        for source in sources.iter().filter(|s| s.state == SourceState::Ready) {
            let x = i64::from(bounds.x()) - i64::from(source.bounds.x());
            let y = i64::from(bounds.y()) - i64::from(source.bounds.y());
            if x < 0
                || y < 0
                || x + i64::from(bounds.width()) > i64::from(source.bounds.width())
                || y + i64::from(bounds.height()) > i64::from(source.bounds.height())
            {
                continue;
            }
            let region = PixelRect::new(x as u32, y as u32, bounds.width(), bounds.height())
                .map_err(|e| RunError::new(ErrorKind::Contract, e.to_string()))?;
            found.push((source.id, region));
        }
        if found.len() != 1 {
            return Err(RunError::new(
                ErrorKind::Unavailable,
                format!(
                    "OCR 窗口必须完整位于一个就绪的采集屏幕内：窗口 {bounds:?}，来源 {sources:?}"
                ),
            ));
        }
        let (source, region) = found[0];
        Ok(QuerySource::Ocr {
            sampler: self.session.sampler.clone(),
            source,
            region,
            window: self.window.clone(),
            input: self.input.clone(),
        })
    }
}

impl QuerySourceProvider for OcrWindow {
    fn resolve<'a>(&'a self, operation: &'a Operation) -> TaskFuture<'a, QuerySource> {
        Box::pin(async move {
            let mut sequence = self.input.sequence(operation).map_err(Failure::from)?;
            sequence
                .perform(
                    self.window.clone(),
                    argusflow_windows::InputAction::ActivateWindow,
                )
                .await
                .map_err(Failure::from)?;
            self.source()
        })
    }
    fn cleanup<'a>(&'a self, _: &'a Operation) -> TaskFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-workflow-automation/unit/ocr_window.rs"]
mod tests;
