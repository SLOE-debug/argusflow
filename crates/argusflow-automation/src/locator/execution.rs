//! 多来源共用的唯一性、取消及副作用边界。
use crate::source::SourceBackend;
use argusflow_aql::BoundQuery;
use argusflow_core::{Failure, FailureKind, Operation};

pub(super) enum Action<'a> {
    FindAll,
    FindUnique,
    Click,
    TypeText(&'a str),
}
pub(super) async fn execute<B: SourceBackend>(
    backend: &B,
    query: &BoundQuery,
    action: Action<'_>,
    operation: &Operation,
) -> Result<Vec<B::Target>, Failure> {
    if let Action::TypeText(text) = action
        && (text.is_empty() || text.len() > 16_384)
    {
        return Err(Failure::new(
            FailureKind::InvalidInput,
            "aql_input",
            "输入文字必须为 1 到 16384 字节",
        ));
    }
    let mut guard = operation.cancel_on_drop();
    let result = async {
        operation.check("aql_start")?;
        // 每次 API 动作执行前都重新定位，绝不重用前一次动作的候选。
        let targets = backend.find(query, operation).await?;
        operation.check("aql_located")?;
        if !matches!(action, Action::FindAll) && targets.len() != 1 {
            return Err(Failure::new(
                if targets.is_empty() {
                    FailureKind::NotFound
                } else {
                    FailureKind::Ambiguous
                },
                "aql_unique",
                "定位操作要求恰好一个结果；多个结果请显式使用 first 或 nth",
            ));
        }
        match action {
            Action::Click => backend.click(&targets[0], operation).await?,
            Action::TypeText(text) => backend.type_text(&targets[0], text, operation).await?,
            Action::FindAll | Action::FindUnique => {}
        }
        operation.check("aql_complete")?;
        Ok(targets)
    }
    .await
    .map_err(|failure| operation.contextualize(failure));
    if result.is_ok() {
        guard.disarm();
    }
    result
}
#[cfg(test)]
#[path = "../../../../tests/argusflow-automation/unit/locator/execution.rs"]
mod tests;
