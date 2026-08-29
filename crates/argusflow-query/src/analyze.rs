use std::collections::BTreeSet;

use argusflow_core::{QueryExpr, SelectorAttribute, UiQuery};
use serde::Serialize;

use crate::{
    Diagnostic, DiagnosticCode, DiagnosticSeverity, QueryBackend, QueryPortability,
    canonicalize_query, normalize_query,
};

/// 与后端编译能力无关的 AQL 语义摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueryAnalysis {
    /// 规范化后的强类型查询。
    normalized: UiQuery,
    /// 可直接用作 cache key 的 canonical AQL。
    canonical_source: String,
    /// 查询是否显式依赖后端 namespace 或 escape hatch。
    portability: QueryPortability,
    /// 与后端 pushdown 规则无关的语义诊断。
    diagnostics: Vec<Diagnostic>,
}

impl QueryAnalysis {
    /// 返回规范化后的只读查询。
    pub const fn normalized(&self) -> &UiQuery {
        &self.normalized
    }

    /// 返回 canonical cache key。
    pub fn canonical_source(&self) -> &str {
        &self.canonical_source
    }

    /// 返回查询可移植性。
    pub const fn portability(&self) -> &QueryPortability {
        &self.portability
    }

    /// 返回不猜测任何 backend compiler 行为的语义诊断。
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// 只分析 AQL 自身语义；后端支持、成本与 residual 必须来自真实 compiler plan。
pub fn analyze_query(query: &UiQuery) -> QueryAnalysis {
    let normalized = normalize_query(query);
    let mut specific_backends = BTreeSet::new();
    collect_specific_backends(&normalized.expression, &mut specific_backends);

    let portability = if specific_backends.is_empty() {
        QueryPortability::Portable
    } else {
        QueryPortability::BackendSpecific {
            backends: specific_backends.iter().copied().collect(),
        }
    };
    let mut diagnostics = Vec::new();
    if !specific_backends.is_empty() {
        diagnostics.push(Diagnostic::global(
            DiagnosticCode::BackendSpecificProperty,
            DiagnosticSeverity::Information,
            None,
        ));
    }
    if !has_root_selection(&normalized.expression) {
        diagnostics.push(Diagnostic::global(
            DiagnosticCode::PotentialMultiMatch,
            DiagnosticSeverity::Warning,
            None,
        ));
    }

    QueryAnalysis {
        canonical_source: canonicalize_query(&normalized),
        normalized,
        portability,
        diagnostics,
    }
}

/// 递归收集显式 backend namespace；此信息仅描述可移植性，不推导能力。
fn collect_specific_backends(expression: &QueryExpr, backends: &mut BTreeSet<QueryBackend>) {
    match expression {
        QueryExpr::Match { matcher } => {
            for predicate in &matcher.predicates {
                match predicate.attribute {
                    SelectorAttribute::Uia(_) => {
                        backends.insert(QueryBackend::WindowsUia);
                    }
                    SelectorAttribute::Dom(_) => {
                        backends.insert(QueryBackend::BrowserCdp);
                    }
                    _ => {}
                }
            }
        }
        QueryExpr::Descendant { ancestor, target }
        | QueryExpr::Child {
            parent: ancestor,
            target,
        } => {
            collect_specific_backends(ancestor, backends);
            collect_specific_backends(target, backends);
        }
        QueryExpr::Any { queries } => {
            for query in queries {
                collect_specific_backends(query, backends);
            }
        }
        QueryExpr::Not { query } | QueryExpr::First { query } | QueryExpr::Nth { query, .. } => {
            collect_specific_backends(query, backends)
        }
        QueryExpr::Nearest { anchor, target, .. } => {
            collect_specific_backends(anchor, backends);
            collect_specific_backends(target, backends);
        }
        QueryExpr::Css { .. } => {
            backends.insert(QueryBackend::BrowserCdp);
        }
    }
}

/// 判断根表达式是否明确约束为单个结果。
const fn has_root_selection(expression: &QueryExpr) -> bool {
    matches!(
        expression,
        QueryExpr::First { .. } | QueryExpr::Nth { .. } | QueryExpr::Nearest { .. }
    )
}
