//! UIA AQL 逻辑计划的 pushdown、缓存与 residual 边界测试。

use argusflow_core::SelectorAttribute;
use argusflow_query::{SupportLevel, parse_query};
use argusflow_windows::uia::{UiaPlanExpr, UiaQueryCompileError, compile_uia_query};

#[test]
fn compiler_pushes_native_predicates_and_caches_residual_attributes() {
    let query = parse_query(r#"button(name matches /保存|Save/i, enabled = true)"#)
        .expect("AQL should parse");
    let plan = compile_uia_query(&query).expect("portable query should compile for UIA");

    assert_eq!(plan.capability.level, SupportLevel::Hybrid);
    let UiaPlanExpr::Match(matcher) = plan.expression else {
        panic!("expected matcher plan");
    };
    assert_eq!(matcher.pushdown.len(), 1);
    assert_eq!(matcher.residual.len(), 1);
    assert_eq!(matcher.cache, vec![SelectorAttribute::Name]);
}

#[test]
fn compiler_preserves_descendant_scope() {
    let query =
        parse_query(r#"window(name contains "微信") >> button(name = "发送", enabled = true)"#)
            .expect("descendant query should parse");
    let plan = compile_uia_query(&query).expect("portable relation should compile");

    assert!(matches!(plan.expression, UiaPlanExpr::Descendant { .. }));
}

#[test]
fn compiler_rejects_dom_specific_query() {
    let query = parse_query(r#"button(dom.test_id = "save-button")"#)
        .expect("DOM namespace should be valid AQL");

    assert_eq!(
        compile_uia_query(&query),
        Err(UiaQueryCompileError::UnsupportedQuery)
    );
}
