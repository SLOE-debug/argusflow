//! 领域工作流转换为编辑器当前无损整数协议，不创建或执行文件。
use crate::document::{EditorData, NodeLayout, WorkflowFile};
use argusflow_ai::InferenceResult;
#[derive(serde::Serialize)]
pub(crate) struct DraftResult {
    pub analysis: argusflow_ai::Analysis,
    pub metrics: argusflow_ai::Metrics,
    pub file: Option<WorkflowFile>,
}
pub(crate) fn into_draft(result: InferenceResult, session_id: &str) -> Result<DraftResult, String> {
    let file = result
        .workflow
        .map(|workflow| {
            let nodes = workflow
                .scopes
                .iter()
                .flat_map(|s| s.nodes.iter().enumerate())
                .map(|(index, n)| {
                    (
                        n.id.clone(),
                        NodeLayout {
                            x: 80.0,
                            y: 80.0 + index as f64 * 120.0,
                            label: n.id.clone(),
                            note: result
                                .analysis
                                .node_evidence
                                .iter()
                                .find(|e| e.node_id == n.id)
                                .map(|e| {
                                    format!(
                                        "录制会话：{session_id}；证据：{}",
                                        e.evidence_ids.join(", ")
                                    )
                                })
                                .unwrap_or_default(),
                        },
                    )
                })
                .collect();
            Ok::<_, String>(WorkflowFile {
                id: uuid::Uuid::new_v4().to_string(),
                definition: crate::document::wire::encode_workflow(&workflow)?,
                editor: EditorData {
                    nodes,
                    edges: Default::default(),
                    drafts: Default::default(),
                },
            })
        })
        .transpose()?;
    Ok(DraftResult {
        analysis: result.analysis,
        metrics: result.metrics,
        file,
    })
}
