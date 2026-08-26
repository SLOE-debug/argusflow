//! UIA AQL 原生计划的 role/property 映射、缓存与 residual 边界测试。

use argusflow_core::{AqlQuery, AutomationAction, AutomationTarget};
use argusflow_query::{SupportLevel, parse_query};
use argusflow_windows::uia::{
    UiaActionCompileError, UiaActionSupport, UiaNativeComparison, UiaNativeValue, UiaPlanExpr,
    UiaProperty, UiaQueryCompileError, UiaQueryPlan, UiaRoleConstraint, compile_uia_action,
    compile_uia_query,
};

#[test]
fn compiler_pushes_native_predicates_and_caches_residual_attributes() {
    let query = parse_query(r#"button(name matches /保存|Save/i, enabled = true)"#)
        .expect("AQL should parse");
    let plan = compile_single(&query);

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
    let plan = compile_single(&query);

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
    let plan = compile_single(&query);

    assert_eq!(plan.capability.level, SupportLevel::Native);
    assert_eq!(plan.capability.branch_path.as_slice(), &[0]);
    assert!(matches!(plan.expression, UiaPlanExpr::Match(_)));
}

#[test]
fn compiler_splits_non_contiguous_any_branches_into_independent_paths() {
    let query = parse_query(
        r#"any(
            button(uia.automation_id = "A"),
            button(dom.test_id = "B"),
            button(uia.automation_id = "C")
        )"#,
    )
    .expect("cross-backend fallback should parse");
    let plans = compile_uia_query(&query).expect("two UIA alternatives should compile");
    let paths = plans
        .iter()
        .map(|plan| plan.capability.branch_path.as_slice())
        .collect::<Vec<_>>();

    assert_eq!(paths, vec![&[0][..], &[2][..]]);
    assert!(
        plans
            .iter()
            .all(|plan| matches!(&plan.expression, UiaPlanExpr::Match(_)))
    );
}

#[test]
fn action_capability_rejects_only_the_unsupported_any_alternative() {
    let source = r#"any(button(name = "Save"), checkbox(name = "Remember"))"#;
    let query = parse_query(source).expect("mixed action capability fallback should parse");
    let plans = compile_uia_query(&query).expect("both UIA query alternatives should compile");
    let action = AutomationAction::Click {
        target: AutomationTarget::query(AqlQuery::v1(source)),
    };
    let mut prepared = plans
        .into_iter()
        .map(|plan| compile_uia_action(&action, plan));

    let supported = prepared
        .next()
        .expect("button branch should exist")
        .expect("button click should remain executable");
    assert_eq!(supported.capability.branch_path.as_slice(), &[0]);
    assert!(matches!(
        prepared.next().expect("checkbox branch should exist"),
        Err(UiaActionCompileError::UnsupportedTargetRole { .. })
    ));
}

#[test]
fn nested_any_uses_flattened_normalized_branch_paths() {
    let query = parse_query(
        r#"any(
            button(uia.automation_id = "A"),
            any(
                button(dom.test_id = "B"),
                button(uia.automation_id = "C")
            )
        )"#,
    )
    .expect("nested fallback should parse");
    let plans = compile_uia_query(&query).expect("nested UIA alternatives should compile");
    let paths = plans
        .iter()
        .map(|plan| plan.capability.branch_path.as_slice())
        .collect::<Vec<_>>();

    assert_eq!(paths, vec![&[0][..], &[2][..]]);
}

#[test]
fn relation_combines_all_branch_choices_into_complete_paths() {
    let query = parse_query(
        r#"any(window(name = "A"), window(name = "B"))
            >> any(button(name = "X"), button(name = "Y"))"#,
    )
    .expect("relation with alternatives on both sides should parse");
    let plans = compile_uia_query(&query).expect("all relation alternatives should compile");
    let paths = plans
        .iter()
        .map(|plan| plan.capability.branch_path.as_slice())
        .collect::<Vec<_>>();

    assert_eq!(
        paths,
        vec![&[0, 0][..], &[0, 1][..], &[1, 0][..], &[1, 1][..]]
    );
    assert!(
        plans
            .iter()
            .all(|plan| matches!(&plan.expression, UiaPlanExpr::Descendant { .. }))
    );
}

#[test]
fn relation_alternative_expansion_stops_at_the_hard_budget() {
    let source = (0..13)
        .map(|index| format!(r#"any(button(name = "A{index}"), button(name = "B{index}"))"#))
        .collect::<Vec<_>>()
        .join(" >> ");
    let query = parse_query(&source).expect("bounded expansion fixture should parse");

    assert!(matches!(
        compile_uia_query(&query),
        Err(UiaQueryCompileError::AlternativeLimitExceeded(error))
            if error.limit() == 4_096
    ));
}

#[test]
fn dialog_compiles_to_window_and_is_dialog_constraint() {
    let query = parse_query(r#"dialog(name contains "Find")"#).expect("dialog query should parse");
    let plan = compile_single(&query);
    let UiaPlanExpr::Match(matcher) = plan.expression else {
        panic!("expected matcher plan");
    };

    assert_eq!(matcher.role, UiaRoleConstraint::Dialog);
}

#[test]
fn visible_true_compiles_to_is_offscreen_false() {
    let query = parse_query("button(visible = true)").expect("visible query should parse");
    let plan = compile_single(&query);
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
    let plan = compile_single(&query);
    let UiaPlanExpr::Match(matcher) = plan.expression else {
        panic!("expected matcher plan");
    };

    assert_eq!(matcher.pushdown[0].property, UiaProperty::AutomationId);
}

#[test]
fn uia_class_name_compiles_native() {
    let query =
        parse_query(r#"pane(uia.class_name = "Scintilla")"#).expect("UIA class query should parse");
    let plan = compile_single(&query);
    let UiaPlanExpr::Match(matcher) = plan.expression else {
        panic!("expected matcher plan");
    };

    assert_eq!(plan.capability.level, SupportLevel::Native);
    assert_eq!(matcher.pushdown[0].property, UiaProperty::ClassName);
}

#[test]
fn uia_identity_properties_compile_to_native_string_comparisons() {
    let query = parse_query(
        r#"menu_item(uia.accelerator_key = "Ctrl+F", uia.access_key = "Alt+F", uia.framework_id = "Win32")"#,
    )
    .expect("UIA identity query should parse");
    let plan = compile_single(&query);
    let UiaPlanExpr::Match(matcher) = plan.expression else {
        panic!("expected matcher plan");
    };

    assert_eq!(
        matcher
            .pushdown
            .iter()
            .map(|predicate| predicate.property)
            .collect::<Vec<_>>(),
        vec![
            UiaProperty::AcceleratorKey,
            UiaProperty::AccessKey,
            UiaProperty::FrameworkId,
        ]
    );
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
    let query_plan = compile_single(&query);
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
    let query_plan = compile_single(&query);
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

/// 取出不含多个可执行 fallback 的唯一 UIA 计划。
fn compile_single(query: &argusflow_core::UiQuery) -> UiaQueryPlan {
    let mut plans = compile_uia_query(query).expect("query should compile for UIA");
    assert_eq!(plans.len(), 1, "query should have one UIA alternative");
    plans.remove(0)
}
