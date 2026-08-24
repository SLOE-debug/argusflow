//! AQL v1 grammar、规范化、格式化和能力分析回归测试。

use argusflow_core::{
    DomAttribute, ElementRole, MatchOperator, PredicateValue, QueryExpr, SelectorAttribute,
    UiaAttribute,
};
use argusflow_query::{
    AqlErrorKind, QueryBackend, QueryPortability, QueryWarningKind, SupportLevel, analyze_query,
    canonicalize_query, format_query, parse_query,
};

#[test]
fn parses_portable_matcher_and_normalizes_predicate_order() {
    let query = parse_query(
        r#"button(
            visible = true,
            name = "保存",
            enabled = true
        )"#,
    )
    .expect("portable matcher should parse");

    assert_eq!(
        canonicalize_query(&query),
        r#"button(enabled=true,name="保存",visible=true)"#
    );
    let QueryExpr::Match { matcher } = query.expression else {
        panic!("expected element matcher");
    };
    assert_eq!(matcher.role, ElementRole::Button);
    assert_eq!(matcher.predicates.len(), 3);
}

#[test]
fn parses_relations_regex_and_explicit_namespaces() {
    let query = parse_query(
        r#"window(name contains "微信")
            >> button(
                name matches /保存|Save/i,
                uia.automation_id = "btnSave",
                uia.class_name != "LegacyButton"
            )"#,
    )
    .expect("relation and namespaced properties should parse");

    let QueryExpr::Descendant { ancestor, target } = query.expression else {
        panic!("expected descendant expression");
    };
    assert!(matches!(*ancestor, QueryExpr::Match { .. }));
    let QueryExpr::Match { matcher } = *target else {
        panic!("expected descendant target matcher");
    };
    assert!(matcher.predicates.iter().any(|predicate| {
        predicate.attribute == SelectorAttribute::Uia(UiaAttribute::AutomationId)
    }));
    assert!(matcher.predicates.iter().any(|predicate| {
        predicate.operator == MatchOperator::Regex
            && matches!(
                &predicate.value,
                PredicateValue::Regex(regex) if regex.case_insensitive
            )
    }));
}

#[test]
fn parses_any_not_first_nth_and_css() {
    let fallback = parse_query(
        r##"any(
            button(key = "save"),
            not(button(name = "取消")),
            first(button(name = "保存")),
            nth(button(dom.test_id = "save-button"), 2),
            css("#toolbar > button:nth-child(3)")
        )"##,
    )
    .expect("v1 combinators should parse");

    let QueryExpr::Any { queries } = fallback.expression else {
        panic!("expected any expression");
    };
    assert_eq!(queries.len(), 5);
    assert!(matches!(&queries[1], QueryExpr::Not { .. }));
    assert!(matches!(&queries[2], QueryExpr::First { .. }));
    assert!(matches!(&queries[3], QueryExpr::Nth { .. }));
    assert!(matches!(&queries[4], QueryExpr::Css { .. }));
}

#[test]
fn canonical_form_deduplicates_predicates_and_any_branches() {
    let query = parse_query(
        r#"any(
            button(name = "保存", enabled = true, name = "保存"),
            any(button(name = "保存"), button(name = "保存")),
            button(name = "取消")
        )"#,
    )
    .expect("nested any should parse");

    assert_eq!(
        canonicalize_query(&query),
        r#"any(button(enabled=true,name="保存"),button(name="保存"),button(name="取消"))"#
    );
}

#[test]
fn pretty_formatter_emits_stable_aql() {
    let query =
        parse_query(r#"button(name="保存",enabled=true)"#).expect("compact matcher should parse");

    assert_eq!(
        format_query(&query),
        "button(\n    enabled = true,\n    name = \"保存\"\n)"
    );
}

#[test]
fn pretty_formatter_round_trips_nested_queries() {
    let query = parse_query(
        r#"any(window(name contains "微信") >> button(name = "发送"), nth(button(name = "确定"), 2))"#,
    )
    .expect("nested query should parse");
    let formatted = format_query(&query);
    let reparsed = parse_query(&formatted).expect("formatted AQL should remain valid");

    assert_eq!(canonicalize_query(&reparsed), canonicalize_query(&query));
}

#[test]
fn rejects_css_attribute_syntax_with_targeted_help() {
    let error = parse_query(r#"button[name="保存"]"#).expect_err("CSS syntax must be rejected");

    assert_eq!(error.kind, AqlErrorKind::CssSyntax);
    assert!(error.help.is_some_and(|help| help.contains("css(")));
}

#[test]
fn rejects_css_operator_and_invalid_predicate_types() {
    let css_operator =
        parse_query(r#"button(name ~= "保存")"#).expect_err("CSS operator must be rejected");
    assert_eq!(css_operator.kind, AqlErrorKind::UnknownOperator);

    let boolean_contains = parse_query("button(enabled contains \"true\")")
        .expect_err("boolean contains must fail type checking");
    assert_eq!(boolean_contains.kind, AqlErrorKind::InvalidPredicate);

    let string_regex = parse_query(r#"button(name matches "保存")"#)
        .expect_err("matches requires a regex literal");
    assert_eq!(string_regex.kind, AqlErrorKind::UnexpectedToken);
}

#[test]
fn rejects_invalid_regex_and_selection_arguments() {
    let invalid_regex =
        parse_query(r#"button(name matches /[/)"#).expect_err("invalid regex must fail early");
    assert_eq!(invalid_regex.kind, AqlErrorKind::InvalidRegex);

    let empty_any =
        parse_query("any()").expect_err("any must preserve a meaningful fallback contract");
    assert_eq!(empty_any.kind, AqlErrorKind::InvalidArgument);

    let zero_index =
        parse_query("nth(button(), 0)").expect_err("nth is one-based and rejects zero");
    assert_eq!(zero_index.kind, AqlErrorKind::InvalidArgument);
}

#[test]
fn analyzer_marks_portable_regex_as_hybrid_where_residual_filter_is_needed() {
    let query = parse_query(r#"button(name matches /保存|Save/i, enabled = true)"#)
        .expect("portable regex should parse");
    let analysis = analyze_query(&query);

    assert_eq!(analysis.portability(), &QueryPortability::Portable);
    assert_eq!(
        analysis.capability(QueryBackend::WindowsUia).level,
        SupportLevel::Hybrid
    );
    assert_eq!(
        analysis.capability(QueryBackend::BrowserCdp).level,
        SupportLevel::Hybrid
    );
    assert!(
        analysis
            .warnings()
            .iter()
            .any(|warning| { warning.kind == QueryWarningKind::RegexResidualFilter })
    );
}

#[test]
fn analyzer_exposes_backend_specific_support_without_silent_fallback() {
    let dom_query =
        parse_query(r#"button(dom.test_id = "save-button")"#).expect("DOM property should parse");
    let analysis = analyze_query(&dom_query);

    assert_eq!(
        analysis.portability(),
        &QueryPortability::BackendSpecific {
            backends: vec![QueryBackend::BrowserCdp]
        }
    );
    assert_eq!(
        analysis.capability(QueryBackend::WindowsUia).level,
        SupportLevel::Unsupported
    );
    assert_eq!(
        analysis.capability(QueryBackend::BrowserCdp).level,
        SupportLevel::Native
    );

    let QueryExpr::Match { matcher } = &analysis.normalized().expression else {
        panic!("expected normalized matcher");
    };
    assert!(
        matcher.predicates.iter().any(|predicate| {
            predicate.attribute == SelectorAttribute::Dom(DomAttribute::TestId)
        })
    );
}

#[test]
fn analyzer_treats_raw_css_as_browser_only_native_query() {
    let query = parse_query(r##"css("#app > button.primary")"##)
        .expect("raw CSS escape hatch should parse");
    let analysis = analyze_query(&query);

    assert_eq!(
        analysis.capability(QueryBackend::BrowserCdp).level,
        SupportLevel::Native
    );
    assert_eq!(
        analysis.capability(QueryBackend::WindowsUia).level,
        SupportLevel::Unsupported
    );
}
