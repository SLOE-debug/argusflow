//! 一组 OCR 模型和采集线程的创建、回滚与关闭。
use argusflow_capture::FrameSampler;
use argusflow_capture_contracts::{DesktopFrameSource, FrameConfig};
use argusflow_core::{Failure, Operation, OperationOptions};
use argusflow_runtime::RunError;
use argusflow_vision::{OcrConfig, OcrEngine, SampledOcr};
use argusflow_windows::DxgiFrameSource;
use argusflow_workflow::ErrorKind;
use std::{sync::Arc, time::Duration};
pub(crate) struct Session {
    pub sampler: SampledOcr,
    pub frames: Arc<FrameSampler>,
    backend: Arc<DxgiFrameSource>,
    engine: OcrEngine,
}
impl Session {
    pub(super) async fn start(
        dependencies: &std::path::Path,
        capture: FrameConfig,
        operation: &Operation,
    ) -> Result<Self, RunError> {
        let loading = OcrEngine::load_with_options(
            OcrConfig::new(dependencies),
            OperationOptions::new(operation.remaining().min(Duration::from_secs(120)))?,
        );
        tokio::pin!(loading);
        let engine = loop {
            operation.check("ocr_services_load")?;
            tokio::select! {
                result = &mut loading => break result.map_err(Failure::from)?,
                _ = tokio::time::sleep(Duration::from_millis(20).min(operation.remaining())) => {}
            }
        };
        let backend = match DxgiFrameSource::start(capture) {
            Ok(source) => Arc::new(source),
            Err(error) => {
                let mut primary = RunError::new(ErrorKind::Unavailable, error.to_string());
                if let Err(cleanup) = engine.shutdown(OperationOptions::default()).await {
                    primary.secondary.push(Failure::from(cleanup).into());
                }
                return Err(primary);
            }
        };
        let frames = match FrameSampler::new(backend.clone()) {
            Ok(frames) => Arc::new(frames),
            Err(error) => {
                let mut primary = RunError::new(ErrorKind::Unavailable, error.to_string());
                if let Err(cleanup) = backend
                    .shutdown(Operation::new(OperationOptions::default()))
                    .await
                {
                    primary
                        .secondary
                        .push(RunError::new(ErrorKind::Unavailable, cleanup.to_string()));
                }
                if let Err(cleanup) = engine.shutdown(OperationOptions::default()).await {
                    primary.secondary.push(Failure::from(cleanup).into());
                }
                return Err(primary);
            }
        };

        Ok(Self {
            sampler: SampledOcr::new(frames.clone(), engine.clone()),
            frames,
            backend,
            engine,
        })
    }
    pub(super) async fn shutdown(&self, operation: &Operation) -> Result<(), RunError> {
        self.sampler.clear_cache();
        // 两项清理同时启动，避免采集清理耗尽预算后模型仍未收到停止信号。
        let options = OperationOptions::new(operation.remaining().min(Duration::from_secs(10)))?;
        let (capture, ocr) = tokio::join!(
            self.backend.shutdown(operation.clone()),
            self.engine.shutdown(options)
        );
        let capture = capture.map_err(|e| RunError::new(ErrorKind::Unavailable, e.to_string()));
        let ocr = ocr.map_err(|e| RunError::from(Failure::from(e)));
        match (capture, ocr) {
            (Err(mut primary), Err(secondary)) => {
                primary.secondary.push(secondary);
                Err(primary)
            }
            (Err(error), _) | (_, Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}
