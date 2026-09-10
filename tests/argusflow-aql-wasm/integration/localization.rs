//! 中文仅为编辑层表示；原文内容、位置和有效性不会丢失。
use argusflow_aql::{EditorPosition, Span, compile, symbols};
use argusflow_aql_wasm::*;
#[test]
fn keywords_round_trip_without_translating_literals_or_parameters() {
    let source = "/* 按钮 name */ 窗口(名称 包含 \"设置😀\") >> 按钮(名称 匹配 /按钮|name/i, 可用 = 真, 值 = $名称)";
    let english = translate(source);
    assert!(
        english
            .source()
            .contains("button(name matches /按钮|name/i, enabled = true, value = $名称)")
    );
    assert_eq!(localize(english.source()).source(), source);
    assert!(compile(english.source()).is_ok());
    assert!(compile(source).is_err());
    let css = "CSS(\"button[name='名称']\")";
    assert_eq!(translate(css).source(), "css(\"button[name='名称']\")");
}
#[test]
fn every_english_symbol_has_a_reversible_localization() {
    for symbol in symbols() {
        let localized = localize(&symbol.name);
        assert_ne!(localized.source(), symbol.name, "{}", symbol.name);
        assert_eq!(translate(localized.source()).source(), symbol.name);
    }
}
#[test]
fn mappings_handle_utf16_crlf_and_emoji() {
    let source = "\r\n按钮(名称 = \"😀保存\", 可用 = )";
    let english = translate(source);
    let start = source.find("可用").unwrap();
    let generated = english.to_generated(Span::new(start, start + "可用".len()));
    assert_eq!(&english.source()[generated.start..generated.end], "enabled");
    assert_eq!(
        english.to_original(generated),
        Span::new(start, start + "可用".len())
    );
    let document = inspect(source);
    assert!(document.english.is_none());
    let error = &document.diagnostics[0];
    let end = source.rfind(')').unwrap();
    assert_eq!(error.range.start, EditorPosition::at(source, end));
}
#[test]
fn incomplete_drafts_do_not_produce_stale_english() {
    assert!(inspect("按钮()").english.is_some());
    for source in ["按", "按钮(", "按钮(名称 = \"未完成", "按钮(可用 = )"] {
        assert!(inspect(source).english.is_none(), "{source}");
    }
    let items = suggest("按", EditorPosition { line: 0, column: 1 });
    assert!(items.iter().any(|item| item.label == "按钮"));
    assert!(
        suggest(
            "按钮(名称 = \"按",
            EditorPosition {
                line: 0,
                column: 11
            }
        )
        .is_empty()
    );
}
#[test]
fn formatting_is_idempotent_and_hover_uses_original_ranges() {
    let source = "按钮( 名称 = \"保存\",可用=真 )";
    let formatted = inspect(source).formatted.unwrap();
    assert_eq!(inspect(&formatted).formatted.unwrap(), formatted);
    let item = hover(source, EditorPosition { line: 0, column: 1 }).unwrap();
    assert!(item.title.contains("button"));
    assert_eq!(item.range.end.column, 2);
}

#[test]
fn hover_describes_functions_attributes_and_typed_parameters() {
    for (source, needle, kind, signature) in [
        (
            "第几个(按钮(), 2)",
            "第几个",
            argusflow_aql::SymbolKind::Function,
            "序号",
        ),
        (
            "文本(文本 = $内容)",
            "文本 =",
            argusflow_aql::SymbolKind::Attribute,
            "文本: 文本",
        ),
        (
            "文本(文本 = $内容)",
            "$内容",
            argusflow_aql::SymbolKind::Parameter,
            "$内容: 文本",
        ),
        (
            "按钮(可用 = $状态)",
            "$状态",
            argusflow_aql::SymbolKind::Parameter,
            "$状态: 布尔",
        ),
        (
            "文本(置信度 >= $阈值)",
            "$阈值",
            argusflow_aql::SymbolKind::Parameter,
            "$阈值: 数值",
        ),
    ] {
        let item = hover(
            source,
            EditorPosition::at(source, source.find(needle).unwrap()),
        )
        .unwrap();
        assert_eq!(item.kind, kind);
        assert!(item.signature.contains(signature), "{}", item.signature);
        assert!(!item.example.is_empty());
        assert!(!item.description.is_empty());
    }
    let item = hover("文本(文本 = \"内容\")", EditorPosition::default()).unwrap();
    assert_eq!(item.kind, argusflow_aql::SymbolKind::Role);
}

#[test]
fn hover_ranges_and_unfinished_parameters_remain_honest() {
    let source = "// 😀\r\n按钮(名称 = \"😀\", 可用 = $状态";
    let offset = source.find("$状态").unwrap();
    let item = hover(source, EditorPosition::at(source, offset + 1)).unwrap();
    assert!(item.signature.contains("类型待确定"));
    assert_eq!(item.range.start, EditorPosition::at(source, offset));
    assert_eq!(item.range.end, EditorPosition::at(source, source.len()));
    for source in [
        "\"按钮\"",
        "/按钮/",
        "// 按钮",
        "/* 按钮 */",
        "CSS(\"按钮\")",
    ] {
        let offset = source.find("按钮").unwrap();
        assert!(
            hover(source, EditorPosition::at(source, offset)).is_none(),
            "{source}"
        );
    }
}

#[test]
fn completion_retains_english_filter_and_existing_parentheses() {
    for (source, label, filter) in [
        ("按", "按钮", "按钮"),
        ("but", "按钮", "button"),
        ("cla", "界面.类名", "class_name"),
        ("界面.类", "界面.类名", "界面.类名"),
    ] {
        let items = suggest(source, EditorPosition::at(source, source.len()));
        let item = items.iter().find(|item| item.label == label).unwrap();
        assert_eq!(item.filter_text, filter);
        assert!(!item.description.is_empty());
    }
    let source = "but(名称 = \"😀\")";
    let items = suggest(source, EditorPosition::at(source, 3));
    let item = items.iter().find(|item| item.label == "按钮").unwrap();
    assert_eq!(item.insert_text, "按钮");
    assert!(!item.insert_as_snippet);
    let item = suggest("but", EditorPosition::at("but", 3))
        .into_iter()
        .find(|item| item.label == "按钮")
        .unwrap();
    assert_eq!(item.insert_text, "按钮($0)");
    assert!(item.insert_as_snippet);
}

#[test]
fn completion_reuses_parameter_names_without_translating_them() {
    let source = "按钮(名称 = $目标, 值 = $目";
    let items = suggest(source, EditorPosition::at(source, source.len()));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].insert_text, "$目标");
    assert_eq!(items[0].kind, argusflow_aql::SymbolKind::Parameter);
}
