use super::*;
use crate::{Attribute, Bindings, Node, QueryTree, Role, Value, ValueType, evaluate};
use argusflow_core::Operation;
use std::collections::BTreeMap;

#[test]
fn chinese_basic_queries_share_typed_parameters() {
    for source in [
        "目标(文本=\"发送\")",
        "目标(文本包含=\"订单\")",
        "按钮(文本=\"保存\", 可用=是)",
        "输入框(名称=\"消息\", 当前值=\"\")",
        "目标(类型=\"按钮\")",
    ] {
        compile_target(source).unwrap();
    }
    let query = compile_target("目标(文本包含=$联系人, 可用=$状态, 置信度 >= $阈值)").unwrap();
    assert_eq!(
        query.parameters(),
        &BTreeMap::from([
            ("联系人".into(), ValueType::Text),
            ("状态".into(), ValueType::Boolean),
            ("阈值".into(), ValueType::Number),
        ])
    );
    let values = Bindings::from([
        ("联系人".into(), Value::Text("\"), 按钮() // $状态".into())),
        ("状态".into(), Value::Boolean(false)),
        ("阈值".into(), Value::Number(0.0)),
    ]);
    let bound = query.bind(&values).unwrap();
    let mut tree = QueryTree::new(4, 1, 4).unwrap();
    tree.push(Node::element(
        None,
        Role::Text,
        BTreeMap::from([
            (Attribute::Text, values["联系人"].clone()),
            (Attribute::Enabled, Value::Boolean(false)),
            (Attribute::Confidence, Value::Number(0.0)),
        ]),
        (),
    ))
    .unwrap();
    assert_eq!(
        evaluate(&bound, &tree, &Operation::unbounded()).unwrap(),
        [0]
    );
    let mut changed = values.clone();
    changed.insert("联系人".into(), Value::Text(String::new()));
    query.bind(&changed).unwrap();
    changed.insert("状态".into(), Value::Text("否".into()));
    assert!(query.bind(&changed).is_err());
    assert_eq!(
        evaluate(&bound, &tree, &Operation::unbounded()).unwrap(),
        [0]
    );
}

#[test]
fn workflow_language_rejects_platform_fields_and_english_without_touching_values() {
    for source in [
        "element()",
        "元素()",
        "按钮(可用=真)",
        "目标(界面.自动化标识=\"x\")",
        "目标(网页.标识=\"x\")",
        "CSS(\"button\")",
    ] {
        assert!(compile_target(source).is_err(), "{source}");
    }
    compile_target("目标(文本=\"element() 网页.标识\", 名称=$button)").unwrap();
    let source = "// 😀\n目标(文本包含=$联系人, 可用=\"否\")";
    let error = compile_target(source).unwrap_err();
    assert_eq!(&source[error.span.start..error.span.end], "\"否\"");
    assert!(compile_target("目标(文本=$同名, 可用=$同名)").is_err());
    let source = "目标(文本包含=123)";
    let error = compile_target(source).unwrap_err();
    assert_eq!(error.code, DiagnosticCode::Type);
    assert_eq!(&source[error.span.start..error.span.end], "123");
}

#[test]
fn unknown_fields_do_not_become_false_or_empty_and_role_is_confirmed() {
    let mut tree = QueryTree::new(4, 1, 4).unwrap();
    tree.push(Node::element(None, Role::Element, BTreeMap::new(), ()))
        .unwrap();
    tree.push(Node::element(
        None,
        Role::Button,
        BTreeMap::from([(Attribute::Text, Value::Text(String::new()))]),
        (),
    ))
    .unwrap();
    for (source, expected) in [
        ("目标(非 可用=是)", vec![]),
        ("目标(可用=否)", vec![]),
        ("目标(文本=\"\")", vec![1]),
        ("目标(类型=\"按钮\")", vec![1]),
    ] {
        let bound = compile_target(source)
            .unwrap()
            .bind(&Bindings::new())
            .unwrap();
        assert_eq!(
            evaluate(&bound, &tree, &Operation::unbounded()).unwrap(),
            expected
        );
    }
}
