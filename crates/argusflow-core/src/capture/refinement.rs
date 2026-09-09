//! 离线精确像素比较契约；后端资源由一个后处理任务独占。
use crate::{CaptureError, EvidenceFrame, InspectionRect};

/// 输入为同尺寸、同坐标的区域像素，输出真正变化的屏幕坐标矩形。
pub trait PixelDiffer: Send {
    /// 精确比较有效颜色通道；不使用阈值、采样或有碰撞的哈希代替像素判断。
    fn compare(
        &mut self,
        previous: &EvidenceFrame,
        current: &EvidenceFrame,
    ) -> Result<Vec<InspectionRect>, CaptureError>;
}
