//! 持久化记录的身份与生命周期。
use super::{Ocr, Structure, Visual};
use argusflow_input_contracts::{InputEvent, WindowContext};
use serde::{Deserialize, Serialize};

/// 日志序号是同一会话内记录身份，零表示无水位。
pub type RecordId = u64;
/// 会话明确状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionPhase {
    /// Hook 已就绪且接受输入。
    Recording,
    /// 停止接受输入，派生任务取消。
    Paused,
    /// 输入关闭，派生任务有界收尾。
    Stopping,
    /// 正常收尾。
    Stopped,
    /// 原始日志可靠性失效。
    Faulted,
    /// 重开时缺少正常终止记录。
    Interrupted,
}
/// 分阶段故障，不能合并成录制失败布尔值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    /// 原始监听／队列。
    Input,
    /// 写入／同步。
    Journal,
    /// UIA。
    Uia,
    /// CDP。
    Cdp,
    /// 历史锚点。
    Anchor,
    /// 图像读取／编码／附件。
    Visual,
    /// OCR。
    Ocr,
    /// 派生处理预算和取消。
    Derivation,
}
/// 单次尝试的明确结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// 已完成。
    Complete,
    /// 用户未启用的可选能力；保留说明，不计为运行时证据缺失。
    NotConfigured,
    /// 已请求的来源不可用或不支持。
    Unavailable,
    /// 无法证明关联／结果。
    Unresolved,
    /// 截止时间到。
    TimedOut,
    /// 队列或字节额度不足。
    BudgetExceeded,
    /// 来源或历史失效。
    Stale,
    /// 因用户暂停或停止而取消。
    Cancelled,
    /// I/O 等失败。
    Failed,
}
impl Outcome {
    /// 是否计入证据缺失；未启用的可选能力与成功记录不产生警告。
    pub fn is_evidence_gap(self) -> bool {
        match self {
            Self::Complete | Self::NotConfigured => false,
            Self::Unavailable
            | Self::Unresolved
            | Self::TimedOut
            | Self::BudgetExceeded
            | Self::Stale
            | Self::Cancelled
            | Self::Failed => true,
        }
    }
}
/// 可迁移会话元信息；绝对路径不进入包。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// 全局随机会话 ID。
    pub id: String,
    /// 格式版本，首版为 1。
    pub format: u32,
    /// 创建 UTC unix 毫秒，仅用于显示。
    pub created_ms: u64,
    /// 原始 QPC 起点。
    pub qpc_origin: i64,
    /// 每秒 QPC tick。
    pub qpc_frequency: u64,
    /// 采样会话 ID，以十进制字符串避免 JS 精度损失。
    pub capture_session: Option<String>,
    /// 生效策略的版本说明。
    pub policy: String,
}
/// 有序记录外壳；存储层另加长度与 BLAKE3 校验。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// 会话内严格递增身份。
    pub id: RecordId,
    /// 记录提交前的原始 QPC；原始发生时间仍在 InputEvent 中。
    pub written_qpc: i64,
    /// 强类型内容。
    pub data: RecordData,
}
/// 相互引用的追加记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordData {
    /// 不可修改的原始事实。
    Raw(InputEvent),
    /// 操作识别结果。
    Interaction(Interaction),
    /// 结构观察。
    Structure(Structure),
    /// 图像及过程观察。
    Visual(Visual),
    /// 固定附件图像的 OCR。
    Ocr(Ocr),
    /// 多对多时间关联，不表达因果。
    Association {
        /// 关联的记录。
        records: Vec<RecordId>,
        /// 可解释关系。
        relation: String,
    },
    /// 包括失败在内，每次结果独立追加。
    Attempt {
        /// 已提交来源记录。
        raw: Vec<RecordId>,
        /// 处理阶段。
        stage: Stage,
        /// 明确结果。
        outcome: Outcome,
        /// 人可读原因。
        reason: String,
    },
    /// 状态边界不与输入混合。
    State {
        /// 新状态。
        phase: SessionPhase,
        /// 状态原因。
        reason: String,
    },
    /// 同步成功后才追加，报告此前的同步水位。
    Durability {
        /// 已 sync_data 的最大序号。
        through: RecordId,
    },
}
/// 操作类型，文字类保留输入意图与确认结果的区别。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteractionKind {
    /// 单击或右键等，button 保存在原始来源。
    Click,
    /// 第二次点击与第一次点击关联，第一条事实不覆盖。
    DoubleClick,
    /// 保留来源轨迹与起止。
    Drag,
    /// 原始增量仍在来源中。
    Scroll,
    /// 组合键。
    Chord,
    /// 桌面系统键释放；保留实际按键，不推断开始菜单是否成功弹出。
    SystemKey,
    /// 文本编辑意图，不能视为最终文本。
    TextUnconfirmed,
    /// IME 过程键，结果未确认。
    ImeUnconfirmed,
    /// 粘贴意图，不读取剪贴板。
    PasteUnconfirmed,
    /// 删除意图。
    DeleteUnconfirmed,
    /// 单次实际编辑值观察，不承诺其因果来源。
    TextObserved,
    /// 前台窗口切换。
    WindowSwitch,
    /// 缺少起点或状态边界造成不完整操作。
    Unresolved,
}
/// 不覆盖原始输入的规范化操作。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interaction {
    /// 源原始记录，含轨迹；最多 2048 条。
    pub raw: Vec<RecordId>,
    /// 归一化类型。
    pub kind: InteractionKind,
    /// QPC 发生区间。
    pub from_qpc: i64,
    /// 最后来源发生时间。
    pub through_qpc: i64,
    /// 窗口边界线索。
    pub window: WindowContext,
    /// 识别依据，明确不确定性。
    pub basis: String,
    /// 双击的第一项操作；消费者展示为组合关系。
    pub related: Option<RecordId>,
}
