//! 将录制流程的三条固定正文改为本次真实列表输入；不改变节点顺序和发送次数。
use super::{Result, baidu::HotList};
use argusflow_workflow::{Action, Expr, Value, ValueType, Workflow};
use std::collections::BTreeMap;

pub fn bind(workflow: &mut Workflow, list: &HotList) -> Result<BTreeMap<String, Value>> {
    let original = workflow
        .scopes
        .iter()
        .flat_map(|s| &s.nodes)
        .filter_map(|n| match &n.action {
            Action::Task { task } if task.type_id == "browser.copy_text" => Some(task),
            _ => None,
        })
        .map(|t| match t.inputs.get("expected") {
            Some(Expr::Literal {
                value: Value::Text(text),
                ..
            }) => Ok(text.clone()),
            _ => Err("录制模板缺少原始复制正文"),
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if original.len() != 3 || list.items.len() < 3 {
        return Err("本 demo 要求三条录制复制动作和至少三条真实热搜".into());
    }
    let items = &list.items[..3];
    let mut bindings = BTreeMap::new();
    for (i, item) in items.iter().enumerate() {
        bindings.insert(format!("hot_title_{i}"), Value::Text(item.title.clone()));
        bindings.insert(
            format!("hot_end_{i}"),
            Value::Int(item.title.encode_utf16().count() as i64),
        );
        let document = items[..=i]
            .iter()
            .map(|v| v.title.as_str())
            .collect::<Vec<_>>()
            .join("\r\n");
        bindings.insert(format!("hot_document_{i}"), Value::Text(document.clone()));
        bindings.insert(
            format!("hot_saved_{i}"),
            Value::Text(format!("{document}\r\n")),
        );
    }
    let (mut copied, mut documents, mut files, mut sent) = (0, 0, 0, 0);
    for node in workflow.scopes.iter_mut().flat_map(|s| &mut s.nodes) {
        let Action::Task { task } = &mut node.action else {
            continue;
        };
        let key = match task.type_id.as_str() {
            "browser.copy_text" => {
                let item = items.get(copied).ok_or("复制节点数量改变")?;
                task.config["query"] =
                    serde_json::json!(format!("CSS({})", serde_json::to_string(&item.selector)?));
                task.inputs.insert(
                    "end".into(),
                    Expr::Input {
                        name: format!("hot_end_{copied}"),
                    },
                );
                let key = format!("hot_title_{copied}");
                copied += 1;
                key
            }
            "window.wait_document" => {
                let key = format!("hot_document_{documents}");
                documents += 1;
                key
            }
            "file.wait_text" => {
                let key = format!("hot_saved_{files}");
                files += 1;
                key
            }
            "clipboard.wait_text" | "demo.wechat_paste_send" => {
                let Some(Expr::Literal {
                    value: Value::Text(expected),
                    ..
                }) = task.inputs.get("expected")
                else {
                    return Err("正文前置条件缺失".into());
                };
                let i = original
                    .iter()
                    .position(|text| text == expected)
                    .ok_or("正文未对应录制条目")?;
                if task.type_id == "demo.wechat_paste_send" {
                    sent += 1;
                }
                format!("hot_title_{i}")
            }
            _ => continue,
        };
        if !bindings.contains_key(&key) {
            return Err("模板的文档验证数量与列表不一致".into());
        }
        task.inputs
            .insert("expected".into(), Expr::Input { name: key });
    }
    if (copied, documents, files, sent) != (3, 3, 3, 3) {
        return Err("模板不满足三次复制、逐条保存和逐条发送契约".into());
    }
    for (name, value) in &bindings {
        workflow.inputs.insert(
            name.clone(),
            if matches!(value, Value::Int(_)) {
                ValueType::Int
            } else {
                ValueType::Text
            },
        );
    }
    workflow.name = "真实百度热搜逐条复制至记事本再发送文件传输助手".into();
    Ok(bindings)
}
