//! CDP AQL 逻辑计划的 DOM fast path 与 AX residual 边界测试。

use argusflow_browser::cdp::{
    CdpCandidateSource, CdpPlanExpr, CdpQueryCompileError, CdpQueryPlan, compile_cdp_query,
};
use argusflow_core::SelectorAttribute;
use argusflow_query::{SupportLevel, parse_query};

#[test]
fn compiler_uses_raw_css_fast_path() {
    let query = parse_query(r##"css("#editor > button.primary")"##).expect("raw CSS should parse");
    let plan = compile_single(&query);

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
    let plan = compile_single(&query);

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
    let plan = compile_single(&query);

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

#[test]
fn compiler_keeps_supported_branch_of_cross_backend_any() {
    let query = parse_query(
        r#"any(
            button(uia.automation_id = "save"),
            button(dom.test_id = "save")
        )"#,
    )
    .expect("cross-backend any should parse");
    let plan = compile_single(&query);

    assert_eq!(plan.capability.level, SupportLevel::Native);
    assert_eq!(plan.capability.branch_path.as_slice(), &[1]);
    assert!(matches!(plan.expression, CdpPlanExpr::Match(_)));
}

#[test]
fn compiler_splits_non_contiguous_any_branches_into_independent_paths() {
    let query = parse_query(
        r#"any(
            button(dom.test_id = "A"),
            button(uia.automation_id = "B"),
            button(dom.test_id = "C")
        )"#,
    )
    .expect("cross-backend fallback should parse");
    let plans = compile_cdp_query(&query).expect("two CDP alternatives should compile");
    let paths = plans
        .iter()
        .map(|plan| plan.capability.branch_path.as_slice())
        .collect::<Vec<_>>();

    assert_eq!(paths, vec![&[0][..], &[2][..]]);
    assert!(
        plans
            .iter()
            .all(|plan| matches!(&plan.expression, CdpPlanExpr::Match(_)))
    );
}

/// 取出不含多个可执行 fallback 的唯一 CDP 计划。
fn compile_single(query: &argusflow_core::UiQuery) -> CdpQueryPlan {
    let mut plans = compile_cdp_query(query).expect("query should compile for CDP");
    assert_eq!(plans.len(), 1, "query should have one CDP alternative");
    plans.remove(0)
}
