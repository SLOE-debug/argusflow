use argusflow_core::{
    ElementMatcher, PredicateValue, PropertyPredicate, QueryExpr, SpatialAnchor, UiQuery,
};

/// 生成语义等价且顺序稳定的查询树，供缓存、分析和 canonical 输出使用。
pub fn normalize_query(query: &UiQuery) -> UiQuery {
    UiQuery::new(normalize_expression(&query.expression))
}

/// 递归规范化表达式，并保持 `any` 的回退优先级。
fn normalize_expression(expression: &QueryExpr) -> QueryExpr {
    match expression {
        QueryExpr::Match { matcher } => QueryExpr::Match {
            matcher: normalize_matcher(matcher),
        },
        QueryExpr::Descendant { ancestor, target } => QueryExpr::Descendant {
            ancestor: Box::new(normalize_expression(ancestor)),
            target: Box::new(normalize_expression(target)),
        },
        QueryExpr::Child { parent, target } => QueryExpr::Child {
            parent: Box::new(normalize_expression(parent)),
            target: Box::new(normalize_expression(target)),
        },
        QueryExpr::Any { queries } => normalize_any(queries),
        QueryExpr::Not { query } => {
            let normalized = normalize_expression(query);
            if let QueryExpr::Not { query: inner } = normalized {
                *inner
            } else {
                QueryExpr::Not {
                    query: Box::new(normalized),
                }
            }
        }
        QueryExpr::First { query } => QueryExpr::First {
            query: Box::new(normalize_expression(query)),
        },
        QueryExpr::Nth { query, index } => QueryExpr::Nth {
            query: Box::new(normalize_expression(query)),
            index: *index,
        },
        QueryExpr::Nearest {
            anchor,
            target,
            direction,
            index,
            metric,
        } => QueryExpr::Nearest {
            anchor: normalize_spatial_anchor(anchor),
            target: Box::new(normalize_expression(target)),
            direction: *direction,
            index: *index,
            metric: *metric,
        },
        QueryExpr::Css { selector } => QueryExpr::Css {
            selector: selector.trim().to_owned(),
        },
    }
}

/// 只递归规范化元素锚点；viewport 锚点已经是规范化值。
fn normalize_spatial_anchor(anchor: &SpatialAnchor) -> SpatialAnchor {
    match anchor {
        SpatialAnchor::Element { query } => SpatialAnchor::Element {
            query: Box::new(normalize_expression(query)),
        },
        SpatialAnchor::ViewportCorner { position } => SpatialAnchor::ViewportCorner {
            position: *position,
        },
        SpatialAnchor::ViewportEdge { side } => SpatialAnchor::ViewportEdge { side: *side },
    }
}

/// 对 AND 谓词排序并移除完全重复的条件。
fn normalize_matcher(matcher: &ElementMatcher) -> ElementMatcher {
    let mut predicates = matcher.predicates.clone();
    predicates.sort_by_key(predicate_sort_key);
    predicates.dedup();
    ElementMatcher {
        role: matcher.role,
        predicates,
    }
}

/// 扁平化嵌套 `any`，同时按首次出现位置去重。
fn normalize_any(queries: &[QueryExpr]) -> QueryExpr {
    let mut flattened = Vec::new();
    for query in queries {
        let normalized = normalize_expression(query);
        if let QueryExpr::Any { queries: nested } = normalized {
            flattened.extend(nested);
        } else {
            flattened.push(normalized);
        }
    }

    let mut deduplicated = Vec::new();
    for query in flattened {
        if !deduplicated.contains(&query) {
            deduplicated.push(query);
        }
    }
    if deduplicated.len() == 1 {
        deduplicated
            .pop()
            .expect("the length check proves one normalized any branch exists")
    } else {
        QueryExpr::Any {
            queries: deduplicated,
        }
    }
}

/// 构造与 canonical 表达一致的谓词排序键。
fn predicate_sort_key(predicate: &PropertyPredicate) -> (String, String, String) {
    let value = match &predicate.value {
        PredicateValue::Text(value) => value.clone(),
        PredicateValue::Boolean(value) => value.to_string(),
        PredicateValue::Regex(regex) => format!(
            "/{}/{}",
            regex.pattern,
            if regex.case_insensitive { "i" } else { "" }
        ),
        PredicateValue::Parameter(parameter) => format!("${}", parameter.name),
    };
    (
        predicate.attribute.to_string(),
        predicate.operator.to_string(),
        value,
    )
}
