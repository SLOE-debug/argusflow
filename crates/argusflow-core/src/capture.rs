//! 屏幕采集身份、订阅语义和生命周期；不依赖原生图形 API。

use serde::{Deserialize, Serialize};

pub mod changes;
mod error;
pub mod frame;
pub mod image;
pub mod refinement;
mod update;
pub use error::CaptureError;
pub use update::{ScreenCaptureSource, ScreenCaptureUpdate};

/// 一次采集主机生命周期内唯一的来源身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CaptureSourceId(pub u64);

/// 来源内单调增加的像素状态版本；静止观察不会增加版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CaptureRevision(pub u64);

/// 重建或拓扑改变后的新一代来源，禁止跨代应用增量。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureGeneration(pub u64);

/// 消费者获取画面事实的明确策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureDelivery {
    /// 每个变化按原始顺序交付，历史缺失必须报错。
    Ordered,
    /// 获取最新像素，同时累计游标之后的全部变化范围。
    Latest,
}

/// 相对同一单调时钟起点的微秒时间，呈现与冻结分别记录。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureTiming {
    /// 原生来源提供的呈现时刻。
    pub presented_us: u64,
    /// 像素成为独立不可变内存的时刻。
    pub frozen_us: u64,
}

/// 不含像素和用户输入的采集失效原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureFailure {
    /// 所请求的版本已超出有界历史。
    HistoryGap,
    /// 图形设备或桌面不可访问。
    Unavailable,
    /// 来源已明确停止。
    Closed,
    /// 像素或队列达到内存预算。
    Capacity,
    /// 原生来源报告合并了未处理的帧。
    AccumulatedFrames,
    /// 编码或持久化失败。
    Storage,
}
