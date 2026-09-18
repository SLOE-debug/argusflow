//! 同一份可导入文件用于结构验收和显式启用的真实桌面执行。
use crate::document::{WorkflowFile, compilation, storage};
use argusflow_runtime::{NodeRegistry, prepare};
use argusflow_workflow_automation::{AutomationHost, register_automation};
#[cfg(windows)]
#[path = "../support/notepad_uia.rs"]
mod inspect;

fn document() -> WorkflowFile {
    serde_json::from_str(include_str!(
        "../../argusflow-workflow-automation/fixtures/notepad-ocr-while.workflow.json"
    ))
    .unwrap()
}
#[test]
fn notepad_workflow_is_importable_and_prepares() {
    let file = document();
    storage::validate(&file).unwrap();
    let definition = compilation::compile(&file).unwrap();
    assert_eq!(
        definition
            .scopes
            .iter()
            .flat_map(|s| &s.nodes)
            .filter(|n| matches!(n.action, argusflow_workflow::Action::While { .. }))
            .count(),
        1
    );
    let mut registry = NodeRegistry::new();
    register_automation(&mut registry, AutomationHost::default()).unwrap();
    prepare(definition, &registry).unwrap_or_else(|e| panic!("{e:#?}"));
}

#[cfg(windows)]
#[tokio::test]
#[ignore = "显式启动 Notepad++，执行 workflow、真实输入和 OCR；需要本机模型与可见桌面"]
async fn notepad_workflow_runs_with_real_ocr() {
    use argusflow_core::OperationOptions;
    use argusflow_runtime::{RunInputs, RunOptions, WorkflowEngine};
    use argusflow_windows::{InputService, UiaRuntime};
    use argusflow_workflow::Value;
    let uia = UiaRuntime::start(Default::default(), OperationOptions::default())
        .await
        .unwrap();
    let input = InputService::new().unwrap();
    let ocr = argusflow_workflow_automation::OcrServices::new(
        std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("ocr"),
        argusflow_capture_contracts::FrameConfig {
            width: 3840,
            height: 2160,
            ..Default::default()
        },
    );
    let mut registry = NodeRegistry::new();
    register_automation(
        &mut registry,
        AutomationHost {
            ocr: ocr.clone(),
            uia: Some(uia.clone()),
            input: Some(input.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    let plan = prepare(compilation::compile(&document()).unwrap(), &registry)
        .unwrap_or_else(|e| panic!("{e:#?}"));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let output = root
        .join("target")
        .join(format!("notepad-workflow-{}.txt", uuid::Uuid::new_v4()));
    let engine = WorkflowEngine::new();
    let inputs = RunInputs {
        values: [(
            "output_path".into(),
            Value::Text(output.to_string_lossy().into_owned()),
        )]
        .into(),
        ..Default::default()
    };
    let mut handle = engine.start(plan, inputs, RunOptions::default()).unwrap();
    let result = handle.wait().await.unwrap();
    let ocr_cleanup = ocr
        .shutdown(&argusflow_core::Operation::new(OperationOptions::default()))
        .await;
    let input_cleanup = input.shutdown(OperationOptions::default()).await;
    let uia_cleanup = uia.shutdown(OperationOptions::default()).await;
    println!("output={} result={result:#?}", output.display());
    input_cleanup.unwrap();
    ocr_cleanup.unwrap();
    uia_cleanup.unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.outputs.get("written_lines"), Some(&Value::Int(3)));
    assert_eq!(result.outputs.get("saved"), Some(&Value::Bool(true)));
    let Some(Value::List(targets)) = result.outputs.get("ocr_target") else {
        panic!("缺少 OCR 定位结果")
    };
    assert_eq!(targets.len(), 1);
    let Some(Value::Record(source)) = result.outputs.get("spatial_preview") else {
        panic!("缺少空间预览")
    };
    assert_eq!(
        source.get("kind"),
        Some(&Value::Text("spatial_preview".into()))
    );
    let Value::List(items) = &source["items"] else {
        panic!("预览列表类型错误")
    };
    let Value::Record(preview) = &items[0] else {
        panic!("预览项类型错误")
    };
    let Value::List(candidates) = &preview["candidates"] else {
        panic!("候选类型错误")
    };
    assert_eq!(candidates.len(), 3);
    let selected = candidates
        .iter()
        .filter_map(|c| match c {
            Value::Record(c) if c["selected"] == Value::Bool(true) => Some(c),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0]["rank"],
        Value::Optional(Some(Box::new(Value::Int(1))))
    );
    assert_eq!(engine.retained_resources().await, 0);
    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        format!("ARGUS ANCHOR{}", "\r\n\r\nARGUS TARGET".repeat(3))
    );
}
