//! 合并鼠标采样后的轨迹事实，不生成 Click、Drag 或任何执行目标。

use crate::MouseButton;
use argusflow_core::ScreenPoint;
use serde::{Deserialize, Serialize};

/// 轨迹保留的真实采样点，不插值或伪造坐标。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MotionPoint {
    /// 原始 Hook 序号，用于追溯保留点。
    pub sequence: u64,
    /// 相对录制开始的毫秒数。
    pub elapsed_ms: u64,
    /// 虚拟屏幕物理坐标，允许负数。
    pub point: ScreenPoint,
}

/// 一段连续鼠标移动的精简表达；首尾点始终保留，采样数量不因简化改变。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointerMotion {
    /// 最后一个源事件序号；外层事件序号仍为首个源事件序号。
    pub end_sequence: u64,
    /// 最后一个源事件的相对时间，毫秒。
    pub ended_ms: u64,
    /// 合并前的原始移动采样数量。
    pub sample_count: usize,
    /// 原始路径累计长度，物理像素；包括回折，不是起终点直线距离。
    pub distance_px: f64,
    /// 本次录制内观察到按下且尚未释放的鼠标键，不猜测录制前的按键状态。
    pub pressed_buttons: Vec<MouseButton>,
    /// 按时间排序的起终点、转折点与时间锚点；简化误差不超过 2 物理像素。
    pub points: Vec<MotionPoint>,
}
