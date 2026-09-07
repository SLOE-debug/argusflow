use super::fixtures::entity;
use crate::*;
use argusflow_core::*;

#[test]
fn stable_identifiers_outrank_dynamic_classes_and_coordinates() {
    let mut entity = entity();
    entity.semantics.test_id = Some("customer-name".into());
    entity.semantics.stable_id = Some("customer".into());
    entity.semantics.class_name = Some("css-a1b2c3d4".into());
    let candidates = synthesize_selectors(
        Some(&entity),
        ResolutionBackend::ManagedCdp,
        Some(ScreenPoint { x: 1, y: 2 }),
    );
    assert_eq!(candidates[0].basis, CandidateBasis::TestId);
    assert_eq!(candidates[1].basis, CandidateBasis::StableId);
    assert_eq!(candidates.last().unwrap().basis, CandidateBasis::Coordinate);
    assert!(
        candidates
            .iter()
            .find(|candidate| candidate.basis == CandidateBasis::DynamicAttribute)
            .unwrap()
            .stability_score
            < 30
    );
    assert_eq!(
        candidates,
        synthesize_selectors(
            Some(&entity),
            ResolutionBackend::ManagedCdp,
            Some(ScreenPoint { x: 1, y: 2 })
        )
    );
}

#[test]
fn every_generated_query_roundtrips_aql_v3_without_injected_syntax() {
    for value in [
        "\"quoted\"",
        "a\\b",
        "line\nreturn\r\ttab",
        "中文🙂",
        r#"")]>>button[name="evil"]"#,
    ] {
        let mut entity = entity();
        entity.semantics.name = Some(value.into());
        entity.semantics.automation_id = Some(value.into());
        entity.ancestors.push(ElementSemantics {
            role: Some(ElementRole::Pane),
            automation_id: Some("UserForm".into()),
            ..Default::default()
        });
        for candidate in synthesize_selectors(Some(&entity), ResolutionBackend::Uia, None) {
            let RecordedSelector::Aql(query) = candidate.selector else {
                panic!("expected AQL");
            };
            assert_eq!(query.language_version, QueryLanguageVersion::V3);
            let parsed = argusflow_query::parse_query(&query.source).unwrap();
            assert_eq!(argusflow_query::canonicalize_query(&parsed), query.source);
        }
    }
}

#[test]
fn observed_ancestor_is_used_and_no_nth_is_invented() {
    let mut entity = entity();
    entity.ancestors.push(ElementSemantics {
        role: Some(ElementRole::Pane),
        automation_id: Some("BillingForm".into()),
        ..Default::default()
    });
    let candidates = synthesize_selectors(Some(&entity), ResolutionBackend::Uia, None);
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.basis == CandidateBasis::StableAncestor)
    );
    assert!(!serde_json::to_string(&candidates).unwrap().contains("nth("));
}

#[test]
fn vision_and_coordinate_never_invent_backend_properties() {
    let candidates = synthesize_selectors(Some(&entity()), ResolutionBackend::Vision, None);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].basis, CandidateBasis::VisualText);
    assert!(synthesize_selectors(None, ResolutionBackend::Coordinate, None).is_empty());
}
