//! 原生来源到共享采集管线的区域更新契约。

use crate::{
    CaptureError, CaptureGeneration, CaptureSourceId, CaptureTiming, EvidenceFrame, InspectionRect,
};

/// 一次来源更新的独立像素；区域使用虚拟屏幕物理坐标。
pub struct ScreenCaptureUpdate {
    /// 当前来源身份。
    pub source: CaptureSourceId,
    /// 来源重建或几何改变后的代数。
    pub generation: CaptureGeneration,
    /// 来源完整范围。
    pub bounds: InspectionRect,
    /// 原生呈现和 CPU 冻结时间。
    pub timing: CaptureTiming,
    /// 首帧为完整基准，后续帧只包含候选区域。
    pub reset: bool,
    /// 原生系统合并的呈现数；大于一不能被解释为逐帧完整。
    pub accumulated_frames: u32,
    /// 无颜色转换的独立区域像素。
    pub patches: Vec<EvidenceFrame>,
}

/// 非阻塞屏幕采集来源；没有新像素时返回空批次。
pub trait ScreenCaptureSource: Send + Sync {
    /// 创建录制结束后独占使用的 GPU 差分会话，不能占用实时采集 context。
    fn create_pixel_differ(&self) -> Result<Box<dyn super::refinement::PixelDiffer>, CaptureError> {
        Err(CaptureError::CaptureUnavailable {
            message: "offline GPU difference is unavailable for this source".into(),
        })
    }
    /// 请求新一轮完整基准并返回来源清单；由应用级采集主机串行调用。
    fn sources(&self) -> Result<Vec<CaptureSourceId>, CaptureError>;
    /// 与呈现时间同源的单调微秒时钟。
    fn clock_us(&self) -> Result<u64, CaptureError>;
    /// 推进原生取帧与异步读回，不等待编码或 OCR。
    fn poll(&self) -> Result<Vec<ScreenCaptureUpdate>, CaptureError>;
    /// 停止提交新 GPU 工作，仅排空已提交读回；bool 表示仍有待完成槽。
    fn drain(&self) -> Result<(Vec<ScreenCaptureUpdate>, bool), CaptureError>;
}
