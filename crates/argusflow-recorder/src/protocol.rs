//! 父子进程的有界 JSON 行协议；大附件不经管道传输。
use crate::{Record, RecordData, Session, SessionPhase};
use serde::{Deserialize, Serialize};
/// 最大 IPC / 日志单条载荷；拒绝无限分配。
pub const MAX_RECORD_BYTES: usize = 1024 * 1024;
/// 启动握手，仅从父进程的私有管道接收。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRequest {
    /// 数据包绝对目录，仅 IPC 使用，不写入元数据。
    pub directory: String,
    /// 时钟与会话元信息。
    pub session: Session,
    /// 忽略此 PID 的录制控制窗口。
    pub excluded_pid: u32,
    /// 可选显式进程范围；生产桌面录制为 None，独立验收只接受专属进程。
    pub included_pid: Option<u32>,
}
/// 父进程命令。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecorderCommand {
    /// 第一个且唯一的启动请求。
    Start(StartRequest),
    /// 暂停／恢复／停止状态转换。
    Transition(SessionPhase),
    /// 先停止 Hook 并排空原始队列；派生结果仍可在最终状态边界前写入。
    SuspendInput,
    /// 已提交原始事实的派生结果；子进程校验引用。
    Append(RecordData),
    /// 存活租约，连续三秒未收到停止接受输入。
    Heartbeat,
}
impl RecorderCommand {
    /// 证据超出单条预算时只保存该阶段失败，不拖垮原始输入管道。
    pub fn evidence(data: RecordData) -> Self {
        if serde_json::to_vec(&data).is_ok_and(|bytes| bytes.len() <= MAX_RECORD_BYTES - 256) {
            return Self::Append(data);
        }
        let (raw, stage) = match data {
            RecordData::Structure(s) => (
                s.raw,
                if s.source == crate::StructureSource::Uia {
                    crate::Stage::Uia
                } else {
                    crate::Stage::Cdp
                },
            ),
            RecordData::Visual(s) => (s.raw, crate::Stage::Visual),
            RecordData::Ocr(s) => (s.raw, crate::Stage::Ocr),
            RecordData::Attempt { raw, stage, .. } => (raw, stage),
            other => return Self::Append(other),
        };
        Self::Append(RecordData::Attempt {
            raw,
            stage,
            outcome: crate::Outcome::BudgetExceeded,
            reason: "证据无法编码或超过单条载荷预算；未发布成功引用，原始输入继续保存".into(),
        })
    }
}
/// 子进程通知，不改变磁盘事实。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecorderNotice {
    /// 安装监听并创建日志后握手。
    Ready,
    /// Hook 已停止接受输入，之前的 Raw 通知已全部排在此确认之前。
    InputSuspended,
    /// 已完成 write_all；不表示 sync_data。
    Written(Box<Record>),
    /// sync_data 成功的水位。
    Synced(u64),
    /// 原始日志已不可靠。
    Fault(String),
    /// 最终状态；进程随后退出。
    Finished,
}
impl RecorderNotice {
    /// 大记录在管道队列中只保存一个拥有者指针。
    pub fn written(record: Record) -> Self {
        Self::Written(Box::new(record))
    }
}
