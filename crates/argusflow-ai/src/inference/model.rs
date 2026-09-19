use argusflow_workflow::Workflow;
use serde::{Deserialize, Serialize};
/// 推断结果与尚未回放的审计说明分离。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceResult {
    /// 编译通过的候选；证据不充分时为空。
    pub workflow: Option<Workflow>,
    /// 可读分析与原始证据引用。
    pub analysis: Analysis,
    /// 宿主填写的调用统计，模型无需输出。
    #[serde(default)]
    pub metrics: Metrics,
}
/// 不能用模型自述代替实际回放验证。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Analysis {
    /// 对录制的整体解释。
    pub summary: String,
    /// 每个节点的来源证据。
    pub node_evidence: Vec<NodeEvidence>,
    /// 无法表达或无法判断的步骤。
    pub unresolved: Vec<Unresolved>,
    /// 运行时需提供的输入或资源。
    pub required_bindings: Vec<Binding>,
    /// 固定 false，直到独立回放验收；模型不得提升它。
    pub replay_ready: bool,
}
/// 节点证据，不把校验节点冒充原始操作。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeEvidence {
    /// 工作流节点 ID。
    pub node_id: String,
    /// 本次录制的十进制记录 ID。
    pub evidence_ids: Vec<String>,
    /// 已确认、仅请求或不确定。
    pub outcome: Outcome,
    /// 冲突或保护节点的解释。
    pub uncertainty: Option<String>,
}
/// 观察的业务结果，不等同模型置信度。
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// 状态变化证据完整。
    Confirmed,
    /// 只看到了请求。
    RequestOnly,
    /// 证据冲突或缺失。
    Uncertain,
}
/// 无法生成的步骤仍需保留来源。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unresolved {
    /// 关联原始记录。
    pub evidence_ids: Vec<String>,
    /// 用户可理解的具体原因。
    pub reason: String,
}
/// 宿主需要补齐的绑定。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    /// 声明名称。
    pub name: String,
    /// 输入或原生资源。
    pub kind: BindingKind,
    /// 为什么需要宿主提供。
    pub reason: String,
}
/// 绑定所属命名空间。
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingKind {
    /// 数据值。
    Input,
    /// 借入原生资源。
    Resource,
}
/// 真实 API 使用统计，不由模型填写。
#[derive(Default, Serialize, Deserialize)]
pub struct Metrics {
    /// 完成的网络轮数。
    pub rounds: u8,
    /// 实际处理的工具请求数。
    pub tool_calls: u8,
    /// 累计传给 API 的图片像素，含历史重复提交。
    pub image_pixels: u64,
    /// 供应商返回的用量对象。
    pub usage: Vec<serde_json::Value>,
}
/// 前端进度，不包含录制正文或密钥。
#[derive(Serialize, Clone)]
pub struct Progress {
    /// 当前对话轮。
    pub round: u8,
    /// 本轮阶段。
    pub stage: ProgressStage,
}
/// 分析过程的有限阶段。
#[derive(Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ProgressStage {
    /// 等待模型响应。
    Analyzing,
    /// 补读本机已录制事实。
    Inspecting,
    /// 校验候选和证据。
    Validating,
}
