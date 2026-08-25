//! AQL 编辑器使用的只读解析、格式化与能力分析命令。

use argusflow_core::AqlQuery;
use argusflow_query::{
    AqlError, BackendQueryCapability, QueryPortability, QueryWarning, analyze_query, format_query,
    parse_stored_query,
};
use serde::Serialize;

/// AQL 编辑器的一次完整检查结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AqlInspection {
    /// 查询已通过语法和语义检查，并附带规范化及能力分析结果。
    Valid {
        /// 可作为缓存键和紧凑预览的规范单行源码。
        canonical_source: String,
        /// 适合写回编辑器的稳定多行源码。
        formatted_source: String,
        /// 查询是否依赖某个后端专用命名空间。
        portability: QueryPortability,
        /// UIA、CDP 与 Vision 的稳定顺序能力列表。
        capabilities: Vec<BackendQueryCapability>,
        /// 不阻止保存但会影响稳定性或成本的分析警告。
        warnings: Vec<QueryWarning>,
    },
    /// 查询无效，诊断携带源码范围、行列和修复建议。
    Invalid {
        /// 可直接投影到前端编辑器的结构化错误。
        diagnostic: AqlError,
    },
}

#[tauri::command]
/// 检查持久化 AQL，并返回格式化源码、可移植性、后端能力和精确诊断。
pub fn inspect_aql(query: AqlQuery) -> AqlInspection {
    let parsed = match parse_stored_query(&query) {
        Ok(parsed) => parsed,
        Err(diagnostic) => return AqlInspection::Invalid { diagnostic },
    };
    let analysis = analyze_query(&parsed);

    AqlInspection::Valid {
        canonical_source: analysis.canonical_source().to_owned(),
        formatted_source: format_query(analysis.normalized()),
        portability: analysis.portability().clone(),
        capabilities: analysis.capabilities().to_vec(),
        warnings: analysis.warnings().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use argusflow_core::AqlQuery;
    use argusflow_query::{QueryBackend, SupportLevel};

    use super::{AqlInspection, inspect_aql};

    #[test]
    fn inspection_returns_formatter_and_capabilities_for_valid_query() {
        let result = inspect_aql(AqlQuery::v1(r#"button(name="保存",enabled=true)"#));

        let AqlInspection::Valid {
            formatted_source,
            capabilities,
            ..
        } = result
        else {
            panic!("valid AQL should produce an analysis");
        };
        assert!(formatted_source.contains("enabled = true"));
        assert!(capabilities.iter().any(|capability| {
            capability.backend == QueryBackend::WindowsUia
                && capability.level == SupportLevel::Native
        }));
    }

    #[test]
    fn inspection_preserves_parser_diagnostic_for_invalid_query() {
        let result = inspect_aql(AqlQuery::v1(r#"button[name="保存"]"#));

        let AqlInspection::Invalid { diagnostic } = result else {
            panic!("invalid AQL should produce a diagnostic");
        };
        assert_eq!(diagnostic.span.line, 1);
        assert!(diagnostic.message.contains("CSS"));
    }
}
