use std::collections::BTreeSet;

use argusflow_core::{MatchOperator, QueryExpr, SelectorAttribute, UiQuery};
use serde::Serialize;

use crate::{
    BackendQueryCapability, QueryBackend, QueryCost, QueryPortability, QueryWarning,
    QueryWarningKind, SupportLevel, canonicalize_query, normalize_query,
};

/// 规范化查询、可移植性、后端能力和静态警告的完整结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueryAnalysis {
    /// 规范化后的强类型查询。
    normalized: UiQuery,
    /// 可直接用作 cache key 的 canonical AQL。
    canonical_source: String,
    /// 查询是否依赖显式后端能力。
    portability: QueryPortability,
    /// UIA、CDP 与 Vision 的稳定顺序能力列表。
    capabilities: Vec<BackendQueryCapability>,
    /// 可能影响稳定性、成本或可移植性的静态诊断。
    warnings: Vec<QueryWarning>,
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

    /// 返回查询可移植性结论。
    pub const fn portability(&self) -> &QueryPortability {
        &self.portability
    }

    /// 返回稳定顺序的只读后端能力列表。
    pub fn capabilities(&self) -> &[BackendQueryCapability] {
        &self.capabilities
    }

    /// 返回只读静态警告列表。
    pub fn warnings(&self) -> &[QueryWarning] {
        &self.warnings
    }

    /// 返回指定后端的能力；Analyzer 始终生成全部后端条目。
    pub fn capability(&self, backend: QueryBackend) -> BackendQueryCapability {
        self.capabilities
            .iter()
            .copied()
            .find(|capability| capability.backend == backend)
            .expect("analysis always includes every query backend")
    }
}

/// 对已解析查询执行规范化、可移植性和后端能力分析。
pub fn analyze_query(query: &UiQuery) -> QueryAnalysis {
    let normalized = normalize_query(query);
    let features = QueryFeatures::collect(&normalized.expression);
    let capabilities = vec![
        analyze_uia(&features),
        analyze_cdp(&features),
        analyze_vision(&features),
    ];
    let portability = portability(&features);
    let mut warnings = warnings(&features, &capabilities);
    warnings.sort_by_key(|warning| (warning.kind as u8, warning.backend));
    warnings.dedup_by(|left, right| {
        left.kind == right.kind && left.backend == right.backend && left.message == right.message
    });

    QueryAnalysis {
        canonical_source: canonicalize_query(&normalized),
        normalized,
        portability,
        capabilities,
        warnings,
    }
}

/// Analyzer 计算能力时使用的语法特征摘要。
#[derive(Debug, Default)]
struct QueryFeatures {
    /// 显式引用的 backend namespace。
    specific_backends: BTreeSet<QueryBackend>,
    /// 是否包含原生 CSS。
    has_css: bool,
    /// 是否包含正则谓词。
    has_regex: bool,
    /// 是否包含无法普遍下推的文本运算符。
    has_residual_text: bool,
    /// 是否包含 portable 布尔属性。
    has_boolean: bool,
    /// 是否包含 Vision v1 无法可靠读取的 portable 属性。
    has_non_visual_property: bool,
    /// 是否包含层级关系。
    has_relation: bool,
    /// 是否包含多分支组合。
    has_any: bool,
    /// 是否包含取反。
    has_not: bool,
    /// 根查询是否明确选择单个结果。
    has_root_selection: bool,
}

impl QueryFeatures {
    /// 遍历表达式并汇总能力相关特征。
    fn collect(expression: &QueryExpr) -> Self {
        let mut features = Self::default();
        features.visit(expression, true);
        features
    }

    /// 递归访问查询树。
    fn visit(&mut self, expression: &QueryExpr, is_root: bool) {
        match expression {
            QueryExpr::Match { matcher } => {
                for predicate in &matcher.predicates {
                    match predicate.attribute {
                        SelectorAttribute::Uia(_) => {
                            self.specific_backends.insert(QueryBackend::WindowsUia);
                        }
                        SelectorAttribute::Dom(_) => {
                            self.specific_backends.insert(QueryBackend::BrowserCdp);
                        }
                        SelectorAttribute::Name | SelectorAttribute::Value => {}
                        SelectorAttribute::Key
                        | SelectorAttribute::Enabled
                        | SelectorAttribute::Visible
                        | SelectorAttribute::Focused
                        | SelectorAttribute::Checked
                        | SelectorAttribute::Selected => self.has_non_visual_property = true,
                    }
                    self.has_boolean |= predicate.attribute.is_boolean();
                    self.has_regex |= predicate.operator == MatchOperator::Regex;
                    self.has_residual_text |= matches!(
                        predicate.operator,
                        MatchOperator::NotEqual
                            | MatchOperator::Contains
                            | MatchOperator::StartsWith
                            | MatchOperator::EndsWith
                            | MatchOperator::Regex
                    );
                }
            }
            QueryExpr::Descendant { ancestor, target }
            | QueryExpr::Child {
                parent: ancestor,
                target,
            } => {
                self.has_relation = true;
                self.visit(ancestor, false);
                self.visit(target, false);
            }
            QueryExpr::Any { queries } => {
                self.has_any = true;
                for query in queries {
                    self.visit(query, false);
                }
            }
            QueryExpr::Not { query } => {
                self.has_not = true;
                self.visit(query, false);
            }
            QueryExpr::First { query } | QueryExpr::Nth { query, .. } => {
                self.has_root_selection |= is_root;
                self.visit(query, false);
            }
            QueryExpr::Css { .. } => {
                self.has_css = true;
                self.specific_backends.insert(QueryBackend::BrowserCdp);
            }
        }
    }
}

/// 分析 UI Automation 的 pushdown 与 residual 边界。
fn analyze_uia(features: &QueryFeatures) -> BackendQueryCapability {
    let level = if features.has_css
        || features
            .specific_backends
            .contains(&QueryBackend::BrowserCdp)
    {
        SupportLevel::Unsupported
    } else if features.has_not || features.has_any {
        SupportLevel::Emulated
    } else if features.has_regex || features.has_residual_text {
        SupportLevel::Hybrid
    } else {
        SupportLevel::Native
    };
    capability(QueryBackend::WindowsUia, level)
}

/// 分析 CDP 的 CSS fast path、AX pushdown 与 residual 边界。
fn analyze_cdp(features: &QueryFeatures) -> BackendQueryCapability {
    let level = if features
        .specific_backends
        .contains(&QueryBackend::WindowsUia)
    {
        SupportLevel::Unsupported
    } else if features.has_not || features.has_any || features.has_relation {
        SupportLevel::Emulated
    } else if features.has_regex || features.has_residual_text || features.has_boolean {
        SupportLevel::Hybrid
    } else {
        SupportLevel::Native
    };
    capability(QueryBackend::BrowserCdp, level)
}

/// 分析 OCR/GUI tree 能够保持的 portable 语义。
fn analyze_vision(features: &QueryFeatures) -> BackendQueryCapability {
    let level = if !features.specific_backends.is_empty()
        || features.has_css
        || features.has_non_visual_property
        || features.has_boolean
        || features.has_not
    {
        SupportLevel::Unsupported
    } else if features.has_relation || features.has_any {
        SupportLevel::Emulated
    } else {
        SupportLevel::Hybrid
    };
    capability(QueryBackend::Vision, level)
}

/// 根据支持等级推导粗粒度成本。
const fn capability(backend: QueryBackend, level: SupportLevel) -> BackendQueryCapability {
    let estimated_cost = match level {
        SupportLevel::Native => QueryCost::Low,
        SupportLevel::Hybrid => QueryCost::Medium,
        SupportLevel::Emulated | SupportLevel::Unsupported => QueryCost::High,
    };
    BackendQueryCapability {
        backend,
        level,
        estimated_cost,
    }
}

/// 从显式 namespace 集合构造可移植性结论。
fn portability(features: &QueryFeatures) -> QueryPortability {
    if features.specific_backends.is_empty() {
        QueryPortability::Portable
    } else {
        QueryPortability::BackendSpecific {
            backends: features.specific_backends.iter().copied().collect(),
        }
    }
}

/// 根据特征和能力结论生成去重前警告。
fn warnings(
    features: &QueryFeatures,
    capabilities: &[BackendQueryCapability],
) -> Vec<QueryWarning> {
    let mut warnings = Vec::new();
    if !features.specific_backends.is_empty() {
        warnings.push(QueryWarning {
            kind: QueryWarningKind::BackendSpecificProperty,
            backend: None,
            message: "查询使用了后端专用属性或原生 escape hatch".to_owned(),
        });
    }
    if features.has_regex {
        warnings.push(QueryWarning {
            kind: QueryWarningKind::RegexResidualFilter,
            backend: None,
            message: "正则谓词需要后端缩小候选集后执行 residual filter".to_owned(),
        });
    }
    if features.has_not || features.has_any {
        warnings.push(QueryWarning {
            kind: QueryWarningKind::ExpensiveTraversal,
            backend: None,
            message: "查询组合器可能需要多次查询或额外树遍历".to_owned(),
        });
    }
    if !features.has_root_selection {
        warnings.push(QueryWarning {
            kind: QueryWarningKind::PotentialMultiMatch,
            backend: None,
            message: "查询可能返回多个元素；执行时将报告 AmbiguousTarget".to_owned(),
        });
    }
    for capability in capabilities
        .iter()
        .filter(|capability| capability.level == SupportLevel::Unsupported)
    {
        warnings.push(QueryWarning {
            kind: QueryWarningKind::UnsupportedBackend,
            backend: Some(capability.backend),
            message: format!("{:?} 无法保证该查询的完整语义", capability.backend),
        });
    }
    warnings
}
