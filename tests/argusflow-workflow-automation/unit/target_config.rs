use super::*;
use serde_json::json;

fn query(config: serde_json::Value, resource_type: &str) -> Workflow {
    let mut document = flow(
        vec![task(
            "query",
            "aql.exists",
            config,
            vec![(
                "联系人",
                Expr::Input {
                    name: "联系人".into(),
                },
            )],
            vec![("scope", "范围")],
            vec![],
        )],
        vec![],
    );
    document.inputs.insert("联系人".into(), ValueType::Text);
    document
        .resources
        .insert("范围".into(), resource_type.into());
    document
}

#[test]
fn chinese_query_and_parameter_types_compile_for_each_explicit_scope() {
    for (platform, resource_type) in [
        ("uia", "automation.window"),
        ("cdp", "automation.page"),
        ("ocr", "automation.query_source"),
    ] {
        assert!(
            prepare(
                query(
                    json!({"platform":platform,"query":"目标(文本=$联系人)"}),
                    resource_type
                ),
                &registry()
            )
            .is_ok()
        );
    }
}

#[test]
fn platform_scope_mismatch_and_missing_parameters_are_rejected() {
    let config = json!({"platform":"cdp","query":"目标(文本=$联系人)"});
    assert!(prepare(query(config.clone(), "automation.window"), &registry()).is_err());
    let mut document = query(config, "automation.page");
    document.inputs.clear();
    assert!(prepare(document, &registry()).is_err());
    for config in [
        json!({"query":"目标(文本=$联系人)"}),
        json!({"platform":"auto","query":"目标(文本=$联系人)"}),
    ] {
        assert!(prepare(query(config, "automation.page"), &registry()).is_err());
    }
}

#[test]
fn input_mode_is_required_and_unsupported_modes_never_execute() {
    for mode in [None, Some("replace"), Some("selection")] {
        let mut config = json!({"platform":"cdp","query":"输入框()"});
        if let Some(mode) = mode {
            config["mode"] = json!(mode);
        }
        let node = task(
            "input",
            "aql.type_text",
            config,
            vec![("text", Expr::text("消息"))],
            vec![("scope", "page")],
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
fn keyboard_nodes_use_the_same_target_scope_and_reject_invalid_keys() {
    for (platform, keys, accepted) in [
        ("uia", "Control+S", true),
        ("uia", "", false),
        ("uia", "Control+Control", false),
        ("uia", "Unknown", false),
        ("ocr", "Control+S", false),
        ("cdp", "Control+S", false),
    ] {
        let node = task(
            "keys",
            "aql.press_keys",
            json!({"platform": platform, "query":"目标(聚焦=是)", "keys": keys}),
            vec![],
            vec![("scope", "window")],
            vec![],
        );
        let mut document = flow(vec![node], vec![]);
        document
            .resources
            .insert("window".into(), "automation.window".into());
        assert_eq!(
            prepare(document, &registry()).is_ok(),
            accepted,
            "{platform}: {keys}"
        );
    }
}
