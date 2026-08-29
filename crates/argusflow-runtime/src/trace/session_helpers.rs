//! Run Trace 会话的输入来源与节点目录推导。

use std::path::{Path, PathBuf};

use argusflow_core::{ValueExpr, ValueSource};

use super::ResolvedInputSource;

pub(super) fn resolved_source(expression: &ValueExpr) -> ResolvedInputSource {
    match expression {
        ValueExpr::Literal { .. } => ResolvedInputSource::Literal,
        ValueExpr::Ref { source, .. } => match source {
            ValueSource::WorkflowInput { key } => {
                ResolvedInputSource::WorkflowInput { key: key.clone() }
            }
            ValueSource::Variable { name } => ResolvedInputSource::Variable { name: name.clone() },
            ValueSource::Node { node_id } => ResolvedInputSource::Node {
                node_id: node_id.clone(),
            },
        },
        ValueExpr::Expression { source } => ResolvedInputSource::Expression {
            expression: source.clone(),
        },
    }
}

pub(super) fn node_directory(root: &Path, sequence: u64, node_id: &str) -> PathBuf {
    let safe_id = node_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    root.join("nodes").join(format!("{sequence:06}_{safe_id}"))
}
