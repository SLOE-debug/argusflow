//! 剪贴板采样与输入仅建立时间关联，不声称操作因果。
use super::RecordId;
use argusflow_input_contracts::ClipboardObservation;
use serde::{Deserialize, Serialize};
/// 系统剪贴板证据，序号与内容由平台服务提供。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clipboard {
    /// 触发本次观察的已提交输入。
    pub raw: Vec<RecordId>,
    /// 请求开始 QPC。
    pub from_qpc: i64,
    /// 请求完成 QPC，不冒充剪贴板写入时间。
    pub through_qpc: i64,
    /// 与上次成功采样比较的事实。
    pub observation: ClipboardObservation,
}
