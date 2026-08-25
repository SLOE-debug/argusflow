//! UIA AQL 原生计划的 role/property 映射、缓存与 residual 边界测试。

use argusflow_core::{AqlQuery, AutomationAction, AutomationTarget};
use argusflow_query::{SupportLevel, parse_query};
use argusflow_windows::uia::{
    UiaActionCompileError, UiaActionSupport, UiaNativeComparison, UiaNativeValue, UiaPlanExpr,
    UiaProperty, UiaQueryCompileError, UiaRoleConstraint, compile_uia_action, compile_uia_query,
};

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
    assert_eq!(matcher.cache.len(), 1);
    assert_eq!(matcher.cache[0].property(), UiaProperty::Name);
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

#[test]
fn compiler_keeps_supported_branch_of_cross_backend_any() {
    let query = parse_query(
        r#"any(
            button(uia.automation_id = "save"),
            button(dom.test_id = "save")
        )"#,
    )
    .expect("cross-backend any should parse");
    let plan = compile_uia_query(&query).expect("UIA branch should keep the any query supported");

    assert_eq!(plan.capability.level, SupportLevel::Native);
    assert_eq!(plan.capability.earliest_supported_branch_index, 0);
    assert!(matches!(plan.expression, UiaPlanExpr::Match(_)));
}

#[test]
fn dialog_compiles_to_window_and_is_dialog_constraint() {
    let query = parse_query(r#"dialog(name contains "Find")"#).expect("dialog query should parse");
    let plan = compile_uia_query(&query).expect("dialog should compile for UIA");
    let UiaPlanExpr::Match(matcher) = plan.expression else {
        panic!("expected matcher plan");
    };

    assert_eq!(matcher.role, UiaRoleConstraint::Dialog);
}

#[test]
fn visible_true_compiles_to_is_offscreen_false() {
    let query = parse_query("button(visible = true)").expect("visible query should parse");
    let plan = compile_uia_query(&query).expect("visible should compile for UIA");
    let UiaPlanExpr::Match(matcher) = plan.expression else {
        panic!("expected matcher plan");
    };

    assert_eq!(matcher.pushdown[0].property, UiaProperty::IsOffscreen);
    assert_eq!(
        matcher.pushdown[0].comparison,
        UiaNativeComparison::Equal(UiaNativeValue::Boolean(false))
    );
}

#[test]
fn key_compiles_to_automation_id() {
    let query = parse_query(r#"button(key = "save")"#).expect("key query should parse");
    let plan = compile_uia_query(&query).expect("key should compile for UIA");
    let UiaPlanExpr::Match(matcher) = plan.expression else {
        panic!("expected matcher plan");
    };

    assert_eq!(matcher.pushdown[0].property, UiaProperty::AutomationId);
}

#[test]
fn uia_class_name_compiles_native() {
    let query =
        parse_query(r#"pane(uia.class_name = "Scintilla")"#).expect("UIA class query should parse");
    let plan = compile_uia_query(&query).expect("UIA class should compile natively");
    let UiaPlanExpr::Match(matcher) = plan.expression else {
        panic!("expected matcher plan");
    };

    assert_eq!(plan.capability.level, SupportLevel::Native);
    assert_eq!(matcher.pushdown[0].property, UiaProperty::ClassName);
}

#[test]
fn row_and_cell_are_not_falsely_reported_native() {
    for source in ["row()", "cell()"] {
        let query = parse_query(source).expect("table role query should parse");

        assert_eq!(
            compile_uia_query(&query),
            Err(UiaQueryCompileError::UnsupportedQuery)
        );
    }
}

#[test]
fn not_is_rejected_until_complement_scope_is_defined() {
    let query = parse_query("not(button())").expect("not query should parse");

    assert_eq!(
        compile_uia_query(&query),
        Err(UiaQueryCompileError::UnsupportedQuery)
    );
}

#[test]
fn checkbox_click_is_rejected_instead_of_being_reported_as_native_invoke() {
    let query = parse_query(r#"checkbox(name = "Enable")"#).expect("checkbox query should parse");
    let query_plan = compile_uia_query(&query).expect("checkbox should remain queryable");
    let action = AutomationAction::Click {
        target: AutomationTarget::query(AqlQuery::v1(r#"checkbox(name = "Enable")"#)),
    };

    assert!(matches!(
        compile_uia_action(&action, query_plan),
        Err(UiaActionCompileError::UnsupportedTargetRole { .. })
    ));
}

#[test]
fn menu_item_click_reports_runtime_pattern_check_in_combined_support() {
    let query = parse_query(r#"menu_item(name = "Search")"#).expect("menu item query should parse");
    let query_plan = compile_uia_query(&query).expect("menu item should remain queryable");
    let action = AutomationAction::Click {
        target: AutomationTarget::query(AqlQuery::v1(r#"menu_item(name = "Search")"#)),
    };
    let prepared =
        compile_uia_action(&action, query_plan).expect("menu item invoke requires instance proof");

    assert_eq!(
        prepared.action_support,
        UiaActionSupport::RequiresRuntimePatternCheck
    );
    assert_eq!(prepared.capability.level, SupportLevel::Hybrid);
}
