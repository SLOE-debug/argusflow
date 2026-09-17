use argusflow_recorder::*;
use std::collections::BTreeMap;
fn structure() -> Structure {
    Structure {
        raw: vec![1],
        source: StructureSource::Uia,
        relation: Relation::After,
        from_qpc: 2,
        through_qpc: 3,
        identity: BTreeMap::new(),
        properties: BTreeMap::from([("name".into(), Property::Text("button".into()))]),
        ancestors: vec![BTreeMap::new()],
        bounds: None,
        truncated: false,
        stale: false,
        sensitive: false,
        target_confirmed: true,
        scope: "test".into(),
    }
}
#[test]
fn before_after_decisions_are_independent_and_result_is_required() {
    let s = structure();
    assert!(matches!(
        decide(Some(&s), Relation::Before, true),
        EvidenceDecision::VisualRequired(_)
    ));
    assert!(matches!(
        decide(Some(&s), Relation::After, true),
        EvidenceDecision::StructuredSufficient(_)
    ));
    assert!(matches!(
        decide(Some(&s), Relation::After, false),
        EvidenceDecision::VisualRequired(_)
    ));
}
#[test]
fn stale_truncated_unknown_or_sensitive_never_claim_sufficiency() {
    let mut s = structure();
    s.truncated = true;
    assert!(matches!(
        decide(Some(&s), Relation::After, true),
        EvidenceDecision::VisualRequired(_)
    ));
    s.truncated = false;
    s.stale = true;
    assert!(matches!(
        decide(Some(&s), Relation::After, true),
        EvidenceDecision::VisualRequired(_)
    ));
    s.sensitive = true;
    assert_eq!(
        decide(Some(&s), Relation::After, true),
        EvidenceDecision::SensitiveOmitted
    );
    assert!(matches!(
        decide(None, Relation::After, true),
        EvidenceDecision::VisualRequired(_)
    ));
}
