//! 可解释的观察范围、图像坐标及历史版本。
use super::RecordId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// 观察与输入的时间关系。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Relation {
    /// 每个raw引用都在事件到达时固定此缓存版本；事件时间见Raw，像素时间见Visual。
    Input,
    /// 有界等待后的逐事件响应采样，raw只包含该响应对应的输入。
    Response,
    /// 可证明存在于输入之前的固定像素。
    Before,
    /// 输入后观察，不宣称是操作前结构。
    After,
    /// 观察窗口中的代表性版本。
    Intermediate,
}
/// 结构来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructureSource {
    /// Windows UIA。
    Uia,
    /// 显式绑定页面。
    Cdp,
}
/// 普通属性值，禁止原生 COM 和 JS 对象句柄进入模型。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Property {
    /// 文字。
    Text(String),
    /// 开关状态。
    Bool(bool),
    /// 数值属性。
    Number(f64),
    /// 有界页面事件与上下文列表。
    List(Vec<Property>),
    /// 可解释的嵌套属性，不把 JSON 塞入字符串。
    Object(BTreeMap<String, Property>),
    /// 来源明确返回空值。
    Null,
}
/// 结构观察，身份仅在记录范围内有效。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Structure {
    /// 来源输入。
    pub raw: Vec<RecordId>,
    /// UIA/CDP。
    pub source: StructureSource,
    /// 与输入的实际关系。
    pub relation: Relation,
    /// 请求到完成的 QPC 不确定区间。
    pub from_qpc: i64,
    /// 完成时 QPC，不替代发生时间。
    pub through_qpc: i64,
    /// 会话内身份说明，含窗口／target／frame／文档代际。
    pub identity: BTreeMap<String, String>,
    /// 控件／DOM 属性，密码值不进入此映射。
    pub properties: BTreeMap<String, Property>,
    /// 有界祖先上下文。
    pub ancestors: Vec<BTreeMap<String, Property>>,
    /// 屏幕物理边界 [left, top, right, bottom]；CDP 无法转换时为空。
    pub bounds: Option<[i32; 4]>,
    /// 查询是否到达预算或边界。
    pub truncated: bool,
    /// 观察完成时身份是否已失效。
    pub stale: bool,
    /// 已识别密码字段。
    pub sensitive: bool,
    /// 目标关联是否经过复验。
    pub target_confirmed: bool,
    /// 可解释的范围限制。
    pub scope: String,
}
/// 历史像素身份，不承诺可重新连接。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelVersion {
    /// 采样会话，十进制字符串。
    pub session: String,
    /// 显示来源。
    pub source: u64,
    /// 来源代际。
    pub generation: u64,
    /// 像素修订。
    pub revision: u64,
}
/// 图片独立发布后的引用；路径相对数据包根。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    /// 内容 BLAKE3，即附件稳定身份。
    pub hash: String,
    /// 相对路径。
    pub path: String,
    /// 编码字节数。
    pub bytes: u64,
    /// 图像宽高。
    pub size: [u32; 2],
}
/// 截图决策，不把无变化与没有图片混淆。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceDecision {
    /// 结构满足全部规则，给出规则名称。
    StructuredSufficient(String),
    /// 需要像素补充，给出原因。
    VisualRequired(String),
    /// 明确隐私策略省略。
    SensitiveOmitted,
}
/// 视觉终态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualStatus {
    /// 输入关联的不可变缓存图像，保留实际像素呈现时间。
    Anchored,
    /// 输入结束后取得的完整屏幕采样，不宣称业务完成或像素稳定。
    Observed,
}
/// 单个屏幕的独立观察，多屏不宣称原子性。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Visual {
    /// 来源输入，观察可被多项操作引用。
    pub raw: Vec<RecordId>,
    /// Input事件关联或独立的前／后／中间观察。
    pub relation: Relation,
    /// 像素版本。
    pub version: PixelVersion,
    /// 固定像素的系统呈现时间；初始化无法确定时为空。
    pub presented_ns: Option<u64>,
    /// 该版本采集成功时间，不是操作发生时间。
    pub acquired_ns: u64,
    /// 固定像素复制／比较完成时间。
    pub frozen_ns: u64,
    /// 观察起点相对 QPC 域的纳秒。
    pub from_ns: u64,
    /// 处理水位纳秒。
    pub through_ns: u64,
    /// 显示器物理范围 [x,y,width,height]；附件像素按此范围线性映射。
    pub region: [u32; 4],
    /// 屏幕物理原点。
    pub screen_origin: [i32; 2],
    /// 来源有效 DPI；附件另按宽高等比缩放。
    pub dpi: [u32; 2],
    /// 终态。
    pub status: VisualStatus,
    /// 前后分别评估的截图决定。
    pub decision: EvidenceDecision,
    /// 只有完整发布才存在。
    pub image: Option<Attachment>,
}
/// OCR 的原图局部坐标文字块。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrBlock {
    /// 识别文字。
    pub text: String,
    /// 引擎置信度，不代表结构充分性。
    pub confidence: f32,
    /// 原图局部四边形。
    pub polygon: [[f32; 2]; 4],
}
/// OCR 与固定图像的关联。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ocr {
    /// 来源输入。
    pub raw: Vec<RecordId>,
    /// 已发布附件身份。
    pub image_hash: String,
    /// 明确像素版本。
    pub version: PixelVersion,
    /// 缩放局部四边形后加上的物理屏幕原点。
    pub screen_origin: [i32; 2],
    /// OCR 附件尺寸；每个坐标先乘 screen_size / image_size。
    pub image_size: [u32; 2],
    /// 附件对应的物理屏幕尺寸。
    pub screen_size: [u32; 2],
    /// 原始识别结果。
    pub blocks: Vec<OcrBlock>,
}
