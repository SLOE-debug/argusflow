//! 取图 Future 和来源有效性契约，不持有 GPU 历史版本。
use crate::CaptureResult;
use std::{future::Future, pin::Pin};

/// 不绑定执行器的可发送异步操作。
pub type CaptureFuture<T> = Pin<Box<dyn Future<Output = CaptureResult<T>> + Send + 'static>>;

/// 固定图像的来源是否仍有效；设备重建或关闭立即使旧结果失效。
pub trait SourceValidity: Send + Sync {
    /// 仅检查来源身份和生命周期，不代表图像内容仍未改变。
    fn valid(&self) -> bool;
}
