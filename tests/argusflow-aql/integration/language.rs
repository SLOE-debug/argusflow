//! 语言和快照查询的跨平台契约回归。
use argusflow_aql::*;
use argusflow_core::{Operation, OperationOptions};
use std::collections::BTreeMap;

fn bound(source: &str) -> BoundQuery {
    compile(source).unwrap().bind(&Bindings::new()).unwrap()
}
fn tree() -> QueryTree<usize> {
    let mut tree = QueryTree::new(100, 20, 20).unwrap();
    for (parent, role, name, enabled) in [
        (None, Role::Window, "设置", true),
        (Some(0), Role::Pane, "工具栏", true),
        (Some(1), Role::Button, "保存", true),
        (Some(0), Role::Button, "取消", false),
        (Some(0), Role::TextBox, "内容😀", true),
    ] {
        let id = tree.len();
        tree.push(Node::element(
            parent,
            role,
            BTreeMap::from([
                (Attribute::Name, Value::Text(name.into())),
                (Attribute::Enabled, Value::Boolean(enabled)),
            ]),
            id,
        ))
        .unwrap();
    }
    tree
}
fn select(source: &str) -> Vec<usize> {
    evaluate(
        &bound(source),
        &tree(),
        &Operation::new(OperationOptions::default()),
    )
    .unwrap()
}
#[test]
fn boolean_precedence_relations_and_explicit_rank() {
    assert_eq!(
        select(
            r#"window(name contains "设置") >> button(name = "取消" or name = "保存" and enabled = true)"#
        ),
        vec![2, 3]
    );
    assert_eq!(select(r#"window() > button()"#), vec![3]);
    assert_eq!(select("element() > button()"), vec![2, 3]);
    assert_eq!(select("first(element() > button())"), vec![2]);
    assert_eq!(select(r#"nth(window() >> button(), 2)"#), vec![3]);
    assert_eq!(
        select(r#"button(not (enabled = false) and name matches /保.*/)"#),
        vec![2]
    );
}
#[test]
fn parameters_are_frozen_and_never_interpolated() {
    let query = compile("button(name = $名称, enabled = $enabled)").unwrap();
    assert!(query.bind(&Bindings::new()).is_err());
    let bindings = BTreeMap::from([
        ("名称".into(), Value::Text(r#"x"),button()"#.into())),
        ("enabled".into(), Value::Boolean(true)),
    ]);
    let query = query.bind(&bindings).unwrap();
    assert!(
        evaluate(
            &query,
            &tree(),
            &Operation::new(OperationOptions::default())
        )
        .unwrap()
        .is_empty()
    );
    assert!(compile("button(name = $x, enabled = $x)").is_err());
}
#[test]
fn formatter_and_lexical_content_are_stable() {
    let source =
        "/*按钮*/ window( name = \"设置😀\" )>>button(name matches /保[存/]/i,enabled=true)// end";
    let formatted = format_query(source).unwrap();
    assert_eq!(format_query(&formatted).unwrap(), formatted);
    assert!(formatted.contains("/*按钮*/"));
    assert!(formatted.contains("/保[存/]/i"));
    assert!(compile("按钮(名称 = \"保存\")").is_err());
    for source in [
        "button(enabled contains \"x\")",
        "nth(button(),0)",
        "button(name matches /[/)",
        "button(name = )",
        "button(enabled = 4)",
        "first(frame(element()))",
        "element() >> frame(element())",
        "frame(shadow(element())) >> button()",
    ] {
        assert!(compile(source).is_err(), "{source}");
    }
}
#[test]
fn utf16_ranges_and_missing_values() {
    let source = "\r\nbutton(name = \"😀中文\")";
    let offset = source.find('中').unwrap();
    let position = EditorPosition::at(source, offset);
    assert_eq!(position.offset(source), offset);
    assert_eq!(position.line, 1);
    assert!(select("button(not value = \"\")").is_empty());
}
#[test]
fn boundaries_are_explicit_and_hosts_must_be_unique() {
    let mut tree = tree();
    let root = tree.push(Node::boundary(1, Boundary::Shadow)).unwrap();
    tree.push(Node::element(Some(root), Role::Button, BTreeMap::new(), 6))
        .unwrap();
    let operation = Operation::new(OperationOptions::default());
    assert_eq!(
        evaluate(&bound("button()"), &tree, &operation).unwrap(),
        vec![2, 3]
    );
    assert_eq!(
        evaluate(&bound("shadow(pane()) >> button()"), &tree, &operation).unwrap(),
        vec![6]
    );
    assert!(evaluate(&bound("shadow(button()) >> button()"), &tree, &operation).is_err());
}
#[test]
fn cancellation_capability_and_resource_limits_are_errors() {
    let operation = Operation::new(OperationOptions::default());
    operation.cancel();
    assert!(evaluate(&bound("button()"), &tree(), &operation).is_err());
    assert!(
        Capabilities::new([Role::Text], [Attribute::Name])
            .check(&bound("button()"))
            .is_err()
    );
    let mut small = QueryTree::new(2, 2, 1).unwrap();
    small
        .push(Node::element(None, Role::Button, BTreeMap::new(), 0))
        .unwrap();
    small
        .push(Node::element(None, Role::Button, BTreeMap::new(), 1))
        .unwrap();
    assert!(
        evaluate(
            &bound("button()"),
            &small,
            &Operation::new(OperationOptions::default())
        )
        .is_err()
    );
    assert!(
        compile(&format!(
            "{}button(){}",
            "first(".repeat(100),
            ")".repeat(100)
        ))
        .is_err()
    );
}
