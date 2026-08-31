//! AQL 参数在 Runtime prepare 阶段的强类型冻结。

use std::collections::{BTreeMap, BTreeSet};

use argusflow_core::{
    ElementMatcher, PredicateValue, PropertyPredicate, QueryExpr, SpatialAnchor, UiQuery,
};
use thiserror::Error;

/// 参数绑定无法完整解析为本次冻结查询。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QueryParameterResolutionError {
    /// 源码引用了未提供的参数。
    #[error("AQL parameter '${name}' has no binding")]
    MissingBinding {
        /// 不含 `$` 前缀的参数名。
        name: String,
    },
    /// 持久化绑定没有被源码使用，通常表示模板字段已经漂移。
    #[error("AQL binding '${name}' is not referenced by the query")]
    UnusedBinding {
        /// 不含 `$` 前缀的参数名。
        name: String,
    },
}

/// 用已由 Runtime 验证为文本的值替换全部参数，并拒绝缺失或多余绑定。
pub fn resolve_query_parameters(
    query: &UiQuery,
    bindings: &BTreeMap<String, String>,
) -> Result<UiQuery, QueryParameterResolutionError> {
    let mut used = BTreeSet::new();
    let expression = resolve_expression(&query.expression, bindings, &mut used)?;
    if let Some(name) = bindings.keys().find(|name| !used.contains(*name)) {
        return Err(QueryParameterResolutionError::UnusedBinding { name: name.clone() });
    }
    Ok(UiQuery::new(expression))
}

/// 返回查询 AST 引用的参数名集合，供工作流静态校验绑定契约。
pub fn query_parameter_names(query: &UiQuery) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    collect_parameter_names(&query.expression, &mut names);
    names
}

/// 递归收集 matcher 中的参数引用。
fn collect_parameter_names(expression: &QueryExpr, names: &mut BTreeSet<String>) {
    match expression {
        QueryExpr::Match { matcher } => {
            for predicate in &matcher.predicates {
                if let PredicateValue::Parameter(parameter) = &predicate.value {
                    names.insert(parameter.name.clone());
                }
            }
        }
        QueryExpr::Descendant { ancestor, target }
        | QueryExpr::Child {
            parent: ancestor,
            target,
        } => {
            collect_parameter_names(ancestor, names);
            collect_parameter_names(target, names);
        }
        QueryExpr::Nearest { anchor, target, .. } => {
            if let SpatialAnchor::Element { query } = anchor {
                collect_parameter_names(query, names);
            }
            collect_parameter_names(target, names);
        }
        QueryExpr::Any { queries } => {
            for query in queries {
                collect_parameter_names(query, names);
            }
        }
        QueryExpr::Not { query } | QueryExpr::First { query } | QueryExpr::Nth { query, .. } => {
            collect_parameter_names(query, names);
        }
        QueryExpr::Css { .. } => {}
    }
}

/// 递归冻结表达式中的 matcher 参数。
fn resolve_expression(
    expression: &QueryExpr,
    bindings: &BTreeMap<String, String>,
    used: &mut BTreeSet<String>,
) -> Result<QueryExpr, QueryParameterResolutionError> {
    Ok(match expression {
        QueryExpr::Match { matcher } => QueryExpr::Match {
            matcher: ElementMatcher {
                role: matcher.role,
                predicates: matcher
                    .predicates
                    .iter()
                    .map(|predicate| resolve_predicate(predicate, bindings, used))
                    .collect::<Result<Vec<_>, _>>()?,
            },
        },
        QueryExpr::Descendant { ancestor, target } => QueryExpr::Descendant {
            ancestor: Box::new(resolve_expression(ancestor, bindings, used)?),
            target: Box::new(resolve_expression(target, bindings, used)?),
        },
        QueryExpr::Child { parent, target } => QueryExpr::Child {
            parent: Box::new(resolve_expression(parent, bindings, used)?),
            target: Box::new(resolve_expression(target, bindings, used)?),
        },
        QueryExpr::Any { queries } => QueryExpr::Any {
            queries: queries
                .iter()
                .map(|query| resolve_expression(query, bindings, used))
                .collect::<Result<Vec<_>, _>>()?,
        },
        QueryExpr::Not { query } => QueryExpr::Not {
            query: Box::new(resolve_expression(query, bindings, used)?),
        },
        QueryExpr::First { query } => QueryExpr::First {
            query: Box::new(resolve_expression(query, bindings, used)?),
        },
        QueryExpr::Nth { query, index } => QueryExpr::Nth {
            query: Box::new(resolve_expression(query, bindings, used)?),
            index: *index,
        },
        QueryExpr::Nearest {
            anchor,
            target,
            direction,
            index,
            metric,
        } => QueryExpr::Nearest {
            anchor: resolve_spatial_anchor(anchor, bindings, used)?,
            target: Box::new(resolve_expression(target, bindings, used)?),
            direction: *direction,
            index: *index,
            metric: *metric,
        },
        QueryExpr::Css { selector } => QueryExpr::Css {
            selector: selector.clone(),
        },
    })
}

/// 冻结元素锚点中的参数；viewport 锚点不携带运行时值。
fn resolve_spatial_anchor(
    anchor: &SpatialAnchor,
    bindings: &BTreeMap<String, String>,
    used: &mut BTreeSet<String>,
) -> Result<SpatialAnchor, QueryParameterResolutionError> {
    Ok(match anchor {
        SpatialAnchor::Element { query } => SpatialAnchor::Element {
            query: Box::new(resolve_expression(query, bindings, used)?),
        },
        SpatialAnchor::ViewportCorner { position } => SpatialAnchor::ViewportCorner {
            position: *position,
        },
        SpatialAnchor::ViewportEdge { side } => SpatialAnchor::ViewportEdge { side: *side },
    })
}

/// 冻结单个属性谓词右值。
fn resolve_predicate(
    predicate: &PropertyPredicate,
    bindings: &BTreeMap<String, String>,
    used: &mut BTreeSet<String>,
) -> Result<PropertyPredicate, QueryParameterResolutionError> {
    let value = match &predicate.value {
        PredicateValue::Parameter(parameter) => {
            let value = bindings.get(&parameter.name).ok_or_else(|| {
                QueryParameterResolutionError::MissingBinding {
                    name: parameter.name.clone(),
                }
            })?;
            used.insert(parameter.name.clone());
            PredicateValue::Text(value.clone())
        }
        value => value.clone(),
    };
    Ok(PropertyPredicate {
        attribute: predicate.attribute,
        operator: predicate.operator,
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_query;

    #[test]
    fn parameter_values_are_frozen_without_source_interpolation() {
        let query = parse_query("text(name = $group_name)").expect("query parses");
        let resolved = resolve_query_parameters(
            &query,
            &BTreeMap::from([("group_name".to_owned(), "中文\\\"群".to_owned())]),
        )
        .expect("binding resolves");

        let QueryExpr::Match { matcher } = resolved.expression else {
            panic!("resolved root should remain a matcher");
        };
        assert_eq!(
            matcher.predicates[0].value,
            PredicateValue::Text("中文\\\"群".to_owned())
        );
    }
}
