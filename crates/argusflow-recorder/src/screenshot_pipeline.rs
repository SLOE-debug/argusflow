//! 结构化事件测试使用的已冻结证据交付契约。
/// 该通道不进行采集；生产录制图像统一进入独立屏幕时间线。
pub(crate) type PendingScreenshot = tokio::sync::oneshot::Receiver<
    Result<crate::ScreenshotEvidence, argusflow_core::InspectionFailure>,
>;
