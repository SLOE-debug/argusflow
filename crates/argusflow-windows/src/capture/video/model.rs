//! 原型录制边界与可序列化诊断。
use serde::Serialize;
use std::{path::PathBuf, time::Duration};

/// 指定一个显示输出进行有限时长录制；目录必须不存在。
pub struct VideoOptions {
    /// 新会话目录，不覆盖已有视频。
    pub directory: PathBuf,
    /// DXGI 适配器枚举索引。
    pub adapter: u32,
    /// 该适配器下活动输出索引。
    pub output: usize,
    /// 最大帧率，1..=60；静态桌面不伪造新呈现时间。
    pub fps: u32,
    /// H.264 目标码率，单位 bit/s。
    pub bitrate: u32,
    /// 会话目录软上限，按秒检查；最多允许一个检查周期与编码尾部超量。
    pub max_file_bytes: u64,
    /// 自动结束的墙钟时长，最多一小时。
    pub duration: Duration,
}
/// 只报告实际完成的提交和落盘；不是“业务操作成功”的证明。
#[derive(Debug, Serialize)]
pub struct VideoReport {
    /// 已向编码器提交的帧数。
    pub frames: u64,
    /// 因纹理池耗尽而未保存的新画面数量。
    pub pool_drops: u64,
    /// DXGI 累积的额外桌面更新数，不等于业务事件丢失数。
    pub accumulated_updates: u64,
    /// 纹理池应用侧预算，不包括驱动内部资源。
    pub texture_bytes: u64,
    /// 最慢一次 WriteSample 的毫秒数。
    pub max_write_ms: u128,
    /// 编码器属性中的硬件标识。
    pub hardware: String,
    /// 完成 Finalize 后视频大小。
    pub video_bytes: u64,
}
/// 平台、持久化及契约失败显式传播，不启用软件回退。
#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    /// 精确标注原生失败阶段，避免只留下没有上下文的HRESULT。
    #[error("{stage}: {source}")]
    Stage {
        /// 失败的原生操作名。
        stage: &'static str,
        /// 保留原始 HRESULT。
        source: windows::core::Error,
    },
    /// 原生 COM/DXGI/Media Foundation 调用失败。
    #[error("原生视频调用失败: {0}")]
    Native(#[from] windows::core::Error),
    /// 显示来源查询失败。
    #[error(transparent)]
    Capture(#[from] argusflow_capture_contracts::CaptureError),
    /// 文件读写失败。
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON 索引写入失败。
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// 不支持的配置或实际运行状态。
    #[error("{0}")]
    Invalid(String),
}
pub(super) type Result<T> = std::result::Result<T, VideoError>;
pub(super) fn required<T>(value: Option<T>, label: &str) -> Result<T> {
    value.ok_or_else(|| VideoError::Invalid(format!("缺少原生对象: {label}")))
}
