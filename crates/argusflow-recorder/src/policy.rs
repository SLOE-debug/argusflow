//! 结构充分性由可解释事实判断，前后分别调用。
use crate::{EvidenceDecision, Relation, Structure};
use argusflow_input_contracts::{InputEvent, InputKind, InputOrigin};

/// Win 松键会触发开始菜单，必须观察释放后的画面；自身注入不触发递归采集。
pub fn requires_evidence(event: InputEvent) -> bool {
    event.origin != InputOrigin::ArgusFlow && !matches!(event.kind, InputKind::Move)
}
/// 只有完整目标、上下文、时效和相应结果都具备时才能省略图片。
pub fn decide(
    structure: Option<&Structure>,
    relation: Relation,
    result_observed: bool,
) -> EvidenceDecision {
    let Some(s) = structure else {
        return EvidenceDecision::VisualRequired("没有同时间范围的结构观察".into());
    };
    if s.sensitive {
        return EvidenceDecision::SensitiveOmitted;
    }
    if s.relation != relation || s.stale || s.truncated || !s.target_confirmed {
        return EvidenceDecision::VisualRequired("结构的时间、身份、目标关联或完整性不足".into());
    }
    if s.ancestors.is_empty() || s.properties.is_empty() || !result_observed {
        return EvidenceDecision::VisualRequired("目标可识别仍不能解释上下文或结果".into());
    }
    EvidenceDecision::StructuredSufficient(
        "目标关联、相关属性、祖先上下文、时效与结果均已观察".into(),
    )
}
