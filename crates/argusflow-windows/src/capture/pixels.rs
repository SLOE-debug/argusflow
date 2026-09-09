//! 安全像素句柄只向所属适配器线程投递工作，不暴露纹理或 context。
use super::{
    lease::{Epoch, MapLease},
    requests::Request,
};
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation};
use std::sync::{Arc, mpsc::SyncSender};
use tokio::sync::oneshot;

pub(super) struct GpuPixels {
    pub map: Arc<MapLease>,
    pub rotation: Rotation,
    pub valid: Arc<Epoch>,
    pub identity: (SourceId, u64),
    pub sender: SyncSender<Request>,
}
impl SnapshotPixels for GpuPixels {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn valid(&self) -> bool {
        self.valid.valid()
    }
    fn read(self: Arc<Self>, region: PixelRect, operation: Operation) -> CaptureFuture<PixelImage> {
        Box::pin(async move {
            self.check(&operation)?;
            let (reply, receiver) = oneshot::channel();
            self.sender
                .try_send(Request::Read {
                    pixels: self.clone(),
                    region,
                    operation: operation.clone(),
                    reply,
                })
                .map_err(queue_error)?;
            receive(receiver, operation).await
        })
    }
    fn compare(
        self: Arc<Self>,
        before: Arc<dyn SnapshotPixels>,
        regions: Vec<PixelRect>,
        operation: Operation,
    ) -> CaptureFuture<PixelChanges> {
        Box::pin(async move {
            self.check(&operation)?;
            let old = before.as_any().downcast_ref::<GpuPixels>().ok_or_else(|| {
                CaptureError::new(FailureKind::InvalidInput, "gpu_compare", "不同像素后端")
            })?;
            old.check(&operation)?;
            if old.identity != self.identity || !Arc::ptr_eq(&old.valid, &self.valid) {
                return Err(CaptureError::new(
                    FailureKind::StaleHandle,
                    "gpu_compare",
                    "像素来源或代际不同",
                ));
            }
            if regions.is_empty() {
                return Ok(PixelChanges::default());
            }
            if regions.len() > 8192 {
                return Err(CaptureError::new(
                    FailureKind::ResourceLimit,
                    "gpu_compare",
                    "比较区域过多",
                ));
            }
            let (reply, receiver) = oneshot::channel();
            self.sender
                .try_send(Request::Compare {
                    pixels: self.clone(),
                    before: old.map.get()?,
                    regions,
                    operation: operation.clone(),
                    reply,
                })
                .map_err(queue_error)?;
            receive(receiver, operation).await
        })
    }
}
impl GpuPixels {
    pub fn check(&self, operation: &Operation) -> CaptureResult<()> {
        operation.check("gpu_snapshot")?;
        if !self.valid.valid() {
            return Err(CaptureError::new(
                FailureKind::StaleHandle,
                "gpu_snapshot",
                "来源代际已撤销",
            ));
        }
        Ok(())
    }
}
fn queue_error(error: std::sync::mpsc::TrySendError<Request>) -> CaptureError {
    let kind = match error {
        std::sync::mpsc::TrySendError::Full(_) => FailureKind::Busy,
        std::sync::mpsc::TrySendError::Disconnected(_) => FailureKind::Closed,
    };
    CaptureError::new(kind, "gpu_request", "GPU 请求队列不可用")
}
async fn receive<T>(
    receiver: oneshot::Receiver<CaptureResult<T>>,
    operation: Operation,
) -> CaptureResult<T> {
    let mut cancel = operation.cancel_on_drop();
    let result = tokio::time::timeout(operation.remaining(), receiver)
        .await
        .map_err(|_| CaptureError::new(FailureKind::Timeout, "gpu_response", "GPU 请求总时限已到"))?
        .map_err(|_| {
            CaptureError::new(
                FailureKind::Unavailable,
                "gpu_response",
                "GPU 工作线程未返回结果",
            )
        })??;
    operation.check("gpu_response")?;
    cancel.disarm();
    Ok(result)
}
