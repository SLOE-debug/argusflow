//! CDP AQL 逻辑计划的 DOM fast path 与 AX residual 边界测试。

use argusflow_browser::cdp::{
    CdpCandidateSource, CdpPlanExpr, CdpQueryCompileError, compile_cdp_query,
};
use argusflow_core::SelectorAttribute;
use argusflow_query::{SupportLevel, parse_query};

#[test]
fn compiler_uses_raw_css_fast_path() {
    let query = parse_query(r##"css("#editor > button.primary")"##).expect("raw CSS should parse");
    let plan = compile_cdp_query(&query).expect("raw CSS should compile for CDP");

    assert_eq!(plan.capability.level, SupportLevel::Native);
    assert!(matches!(
        plan.expression,
        CdpPlanExpr::Css { selector } if selector == "#editor > button.primary"
    ));
}

#[test]
fn compiler_uses_ax_pushdown_and_projects_regex_attribute() {
    let query = parse_query(r#"button(name matches /保存|Save/i, enabled = true)"#)
        .expect("semantic query should parse");
    let plan = compile_cdp_query(&query).expect("portable query should compile for CDP");

    assert_eq!(plan.capability.level, SupportLevel::Hybrid);
    let CdpPlanExpr::Match(matcher) = plan.expression else {
        panic!("expected matcher plan");
    };
    assert_eq!(matcher.source, CdpCandidateSource::AccessibilityTree);
    assert!(matcher.pushdown.is_empty());
    assert_eq!(
        matcher.projected_attributes,
        vec![SelectorAttribute::Name, SelectorAttribute::Enabled]
    );
}

#[test]
fn compiler_uses_dom_source_for_explicit_dom_property() {
    let query =
        parse_query(r#"button(dom.test_id = "save-button")"#).expect("DOM query should parse");
    let plan = compile_cdp_query(&query).expect("DOM query should compile");

    let CdpPlanExpr::Match(matcher) = plan.expression else {
        panic!("expected matcher plan");
    };
    assert_eq!(matcher.source, CdpCandidateSource::Dom);
    assert_eq!(matcher.pushdown.len(), 1);
    assert!(matcher.residual.is_empty());
}

#[test]
fn compiler_rejects_uia_specific_query() {
    let query = parse_query(r#"button(uia.automation_id = "save")"#)
        .expect("UIA namespace should be valid AQL");

    assert_eq!(
        compile_cdp_query(&query),
        Err(CdpQueryCompileError::UnsupportedQuery)
    );
}
