use super::*;
use serde_json::json;

#[test]
fn document_guard_requires_text_projection() {
    for (query, valid) in [("文档(聚焦=是)", false), ("文档(文本 包含 \"\")", true)] {
        let node = task(
            "verify",
            "window.wait_document",
            json!({"query":query}),
            vec![("expected", Expr::text(""))],
            vec![("window", "editor")],
            vec![],
        );
        let mut document = flow(vec![node], vec![]);
        document
            .resources
            .insert("editor".into(), "automation.window".into());
        assert_eq!(prepare(document, &registry()).is_ok(), valid);
    }
}

#[test]
fn verified_copy_uses_page_resource_and_exact_typed_range() {
    let node = task(
        "copy",
        "browser.copy_text",
        json!({"query":"文本(文本=\"测试\")"}),
        vec![
            ("expected", Expr::text("测试")),
            ("start", Expr::int(0)),
            ("end", Expr::int(2)),
        ],
        vec![("page", "page")],
        vec![],
    );
    let mut document = flow(vec![node], vec![]);
    document
        .resources
        .insert("page".into(), "automation.page".into());
    assert!(prepare(document.clone(), &registry()).is_ok());
    document
        .resources
        .insert("page".into(), "automation.browser".into());
    assert!(prepare(document, &registry()).is_err());
}

#[test]
fn browser_copy_accepts_scoped_css_without_relaxing_document_queries() {
    let node = task(
        "copy",
        "browser.copy_text",
        json!({"query":"CSS(\"#hotsearch-content-wrapper > li[data-index='1'] > a.title-content > .title-content-title\")"}),
        vec![
            ("expected", Expr::text("运行时热搜")),
            ("start", Expr::int(0)),
            ("end", Expr::int(5)),
        ],
        vec![("page", "page")],
        vec![],
    );
    let mut document = flow(vec![node], vec![]);
    document
        .resources
        .insert("page".into(), "automation.page".into());
    assert!(prepare(document, &registry()).is_ok());
    assert!(argusflow_aql::compile_target("CSS(\"body\")").is_err());
}

#[test]
fn transfer_queries_reject_unbound_parameters_and_unknown_fields() {
    for config in [
        json!({"query":"文本(文本=$正文)"}),
        json!({"query":"文本()","selector":"body"}),
    ] {
        let node = task(
            "copy",
            "browser.copy_text",
            config,
            vec![
                ("expected", Expr::text("测试")),
                ("start", Expr::int(0)),
                ("end", Expr::int(2)),
            ],
            vec![("page", "page")],
            vec![],
        );
        let mut document = flow(vec![node], vec![]);
        document
            .resources
            .insert("page".into(), "automation.page".into());
        assert!(prepare(document, &registry()).is_err());
    }
}

#[test]
fn clipboard_guard_requires_typed_prior_sequence() {
    let first = task(
        "before",
        "clipboard.checkpoint",
        json!({}),
        vec![],
        vec![],
        vec![],
    );
    let second = task(
        "after",
        "clipboard.wait_text",
        json!({}),
        vec![
            (
                "previous",
                Expr::NodeOutput {
                    node: "before".into(),
                    output: "sequence".into(),
                },
            ),
            ("expected", Expr::text("重复文本")),
        ],
        vec![],
        vec![],
    );
    assert!(prepare(flow(vec![first, second], vec![]), &registry()).is_ok());
}
