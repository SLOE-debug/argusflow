//! 前端状态与启动配置，不包含平台句柄所有权。
use argusflow_recorder::SessionPhase;
use serde::Serialize;
/// 全局可访问的实时状态；完整数据通过分页读取。
#[derive(Debug, Clone, Default, Serialize)]
pub struct RecorderStatus {
    pub session: Option<String>,
    pub phase: Option<SessionPhase>,
    pub elapsed_ms: u64,
    pub operations: u64,
    pub raw_count: u64,
    pub pending: usize,
    pub written: u64,
    pub synced: u64,
    pub input_fault: Option<String>,
    pub evidence_gaps: u64,
    pub evidence_latency_ms: Vec<u64>,
    pub log_latency_us: Vec<u64>,
}
