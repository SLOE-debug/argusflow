//! AQL v1 grammar、规范化、格式化和能力分析回归测试。

use argusflow_core::{
    DomAttribute, ElementRole, MatchOperator, PredicateValue, QueryExpr, SelectorAttribute,
    UiaAttribute,
};
use argusflow_query::{
    AqlErrorKind, DiagnosticCode, EditorPosition, QueryBackend, QueryPortability, analyze_query,
    byte_range_to_editor_range, canonicalize_query, code_actions, completions, format_query,
    format_source, hover, parse_document, parse_query, query_parameter_names,
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
fn parses_stable_uia_identity_properties() {
    let query = parse_query(
        r#"menu_item(uia.accelerator_key = "Ctrl+F", uia.access_key = "Alt+F", uia.framework_id = "Win32")"#,
    )
    .expect("UIA identity properties should parse");
    let QueryExpr::Match { matcher } = query.expression else {
        panic!("expected a single matcher");
    };

    assert_eq!(
        matcher
            .predicates
            .iter()
            .map(|predicate| predicate.attribute)
            .collect::<Vec<_>>(),
        vec![
            SelectorAttribute::Uia(UiaAttribute::AcceleratorKey),
            SelectorAttribute::Uia(UiaAttribute::AccessKey),
            SelectorAttribute::Uia(UiaAttribute::FrameworkId),
        ]
    );
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
fn parses_parameterized_nearest_and_preserves_canonical_named_arguments() {
    let query = parse_query(
        r#"nearest(
            anchor = text(name contains "网络结果"),
            target = text(name = $group_name),
            direction = below,
            index = 2
        )"#,
    )
    .expect("parameterized nearest should parse");

    assert_eq!(
        canonicalize_query(&query),
        r#"nearest(anchor=text(name contains "网络结果"),target=text(name=$group_name),direction=below,index=2,metric=edge_gap)"#
    );
    assert_eq!(
        query_parameter_names(&query),
        std::collections::BTreeSet::from(["group_name".to_owned()])
    );
    assert!(matches!(query.expression, QueryExpr::Nearest { .. }));
}

#[test]
fn nearest_rejects_zero_rank_and_unknown_direction() {
    let zero = parse_query(
        r#"nearest(anchor = text(name = "A"), target = text(name = "B"), direction = below, index = 0)"#,
    )
    .expect_err("nearest rank is one-based");
    assert_eq!(zero.kind, AqlErrorKind::InvalidArgument);

    let unknown = parse_query(
        r#"nearest(anchor = text(name = "A"), target = text(name = "B"), direction = diagonal, index = 1)"#,
    )
    .expect_err("direction is a closed enum");
    assert_eq!(unknown.kind, AqlErrorKind::InvalidArgument);
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
        "button(\n    name = \"保存\",\n    enabled = true\n)"
    );
}

#[test]
fn pretty_formatter_wraps_long_css_without_changing_selector() {
    let source = r##"css("#hotsearch-content-wrapper a.title-content .title-content-title")"##;

    let formatted = format_source(source).expect("long CSS source should format");

    assert_eq!(
        formatted,
        "css(\n    \"#hotsearch-content-wrapper a.title-content .title-content-title\"\n)"
    );
    assert_eq!(
        canonicalize_query(&parse_query(&formatted).expect("formatted CSS should parse")),
        source
    );
}

#[test]
fn source_formatter_preserves_predicate_order_and_duplicates() {
    let source = r#"button(visible=true,name="保存",name="保存",enabled=true)"#;

    assert_eq!(
        format_source(source).expect("valid source should format"),
        "button(\n    visible = true,\n    name = \"保存\",\n    name = \"保存\",\n    enabled = true\n)"
    );
    assert_eq!(
        canonicalize_query(&parse_query(source).expect("valid source should parse")),
        r#"button(enabled=true,name="保存",visible=true)"#
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
    assert_eq!(
        format_source(&formatted).expect("formatted source should remain valid"),
        formatted
    );
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
fn semantic_analysis_does_not_guess_backend_compiler_capability() {
    let query = parse_query(r#"button(name matches /保存|Save/i, enabled = true)"#)
        .expect("portable regex should parse");
    let analysis = analyze_query(&query);

    assert_eq!(analysis.portability(), &QueryPortability::Portable);
    assert!(analysis.diagnostics().iter().all(|diagnostic| {
        diagnostic.code != DiagnosticCode::ResidualFilter
            && diagnostic.code != DiagnosticCode::UnsupportedBackend
    }));
}

#[test]
fn semantic_analysis_only_reports_backend_specific_portability() {
    let dom_query =
        parse_query(r#"button(dom.test_id = "save-button")"#).expect("DOM property should parse");
    let analysis = analyze_query(&dom_query);

    assert_eq!(
        analysis.portability(),
        &QueryPortability::BackendSpecific {
            backends: vec![QueryBackend::BrowserCdp]
        }
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
        analysis.portability(),
        &QueryPortability::BackendSpecific {
            backends: vec![QueryBackend::BrowserCdp]
        }
    );
}

#[test]
fn recovery_document_keeps_tokens_and_returns_multiple_diagnostics() {
    let document = parse_document("button(\n    name = ,\n    enabled = true\n");

    assert!(document.hir.is_none());
    assert!(
        document
            .syntax
            .tokens
            .iter()
            .any(|token| token.text == "button")
    );
    assert!(
        document
            .syntax
            .tokens
            .iter()
            .any(|token| token.text == "enabled")
    );
    assert!(document.diagnostics.len() >= 2);
    assert!(
        document
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == DiagnosticCode::MissingRightParenthesis })
    );
}

#[test]
fn editor_ranges_use_utf16_columns_for_emoji() {
    let source = "button(name = \"保存😀\")";
    let start = source.find('😀').expect("fixture contains emoji");
    let range = byte_range_to_editor_range(source, start, start + '😀'.len_utf8());

    assert_eq!(
        range.start,
        EditorPosition {
            line: 0,
            utf16_column: 17,
        }
    );
    assert_eq!(range.end.utf16_column, 19);
}

#[test]
fn language_service_offers_css_attribute_quick_fix() {
    let actions = code_actions(r#"button[name="保存"]"#);

    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].new_text, "(");
    assert_eq!(actions[1].new_text, ")");
}

#[test]
fn language_service_provides_completion_and_hover_from_rust_tokens() {
    let completion_items = completions(
        "but",
        EditorPosition {
            line: 0,
            utf16_column: 3,
        },
    );
    assert!(
        completion_items
            .iter()
            .any(|item| { item.label == "button" && item.insert_text == "button()" })
    );

    let hover = hover(
        "button()",
        EditorPosition {
            line: 0,
            utf16_column: 2,
        },
    )
    .expect("role token should provide hover");
    assert_eq!(hover.description_code, "aql.hover.role");
}
