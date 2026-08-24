use argusflow_core::{
    ElementMatcher, PredicateValue, PropertyPredicate, QueryExpr, RegexLiteral, UiQuery,
};

use crate::normalize_query;

/// 将查询规范化并输出单行 canonical cache key。
pub fn canonicalize_query(query: &UiQuery) -> String {
    let normalized = normalize_query(query);
    format_compact_expression(&normalized.expression)
}

/// 将查询规范化并输出适合编辑器展示的稳定多行格式。
pub fn format_query(query: &UiQuery) -> String {
    let normalized = normalize_query(query);
    format_pretty_expression(&normalized.expression, 0)
}

/// 输出无无关空白的表达式。
fn format_compact_expression(expression: &QueryExpr) -> String {
    match expression {
        QueryExpr::Match { matcher } => format_compact_matcher(matcher),
        QueryExpr::Descendant { ancestor, target } => format!(
            "{}>>{}",
            format_compact_expression(ancestor),
            format_compact_expression(target)
        ),
        QueryExpr::Child { parent, target } => format!(
            "{}>{}",
            format_compact_expression(parent),
            format_compact_expression(target)
        ),
        QueryExpr::Any { queries } => format!(
            "any({})",
            queries
                .iter()
                .map(format_compact_expression)
                .collect::<Vec<_>>()
                .join(",")
        ),
        QueryExpr::Not { query } => format!("not({})", format_compact_expression(query)),
        QueryExpr::First { query } => format!("first({})", format_compact_expression(query)),
        QueryExpr::Nth { query, index } => {
            format!("nth({},{index})", format_compact_expression(query))
        }
        QueryExpr::Css { selector } => format!("css({})", quote_string(selector)),
    }
}

/// 输出缩进稳定的可读表达式。
fn format_pretty_expression(expression: &QueryExpr, indent: usize) -> String {
    match expression {
        QueryExpr::Match { matcher } => format_pretty_matcher(matcher, indent),
        QueryExpr::Descendant { ancestor, target } => {
            format_relation(ancestor, target, ">>", indent)
        }
        QueryExpr::Child { parent, target } => format_relation(parent, target, ">", indent),
        QueryExpr::Any { queries } => format_query_list("any", queries, indent),
        QueryExpr::Not { query } => format_unary("not", query, indent),
        QueryExpr::First { query } => format_unary("first", query, indent),
        QueryExpr::Nth { query, index } => {
            let inner_indent = indent + 4;
            format!(
                "nth(\n{}{},\n{}{}\n{})",
                " ".repeat(inner_indent),
                format_pretty_expression(query, inner_indent),
                " ".repeat(inner_indent),
                index,
                " ".repeat(indent)
            )
        }
        QueryExpr::Css { selector } => format!("css({})", quote_string(selector)),
    }
}

/// 输出单行 matcher。
fn format_compact_matcher(matcher: &ElementMatcher) -> String {
    let predicates = matcher
        .predicates
        .iter()
        .map(format_predicate)
        .collect::<Vec<_>>()
        .join(",");
    format!("{}({predicates})", matcher.role)
}

/// 输出属性逐行排列的 matcher。
fn format_pretty_matcher(matcher: &ElementMatcher, indent: usize) -> String {
    if matcher.predicates.is_empty() {
        return format!("{}()", matcher.role);
    }

    let predicate_indent = " ".repeat(indent + 4);
    let mut output = format!("{}(\n", matcher.role);
    for (index, predicate) in matcher.predicates.iter().enumerate() {
        output.push_str(&predicate_indent);
        output.push_str(&format_predicate(predicate));
        if index + 1 < matcher.predicates.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str(&" ".repeat(indent));
    output.push(')');
    output
}

/// 输出层级关系，并将运算符放在下一行以突出搜索范围。
fn format_relation(
    ancestor: &QueryExpr,
    target: &QueryExpr,
    operator: &str,
    indent: usize,
) -> String {
    let relation_indent = indent + 4;
    format!(
        "{}\n{}{} {}",
        format_pretty_expression(ancestor, indent),
        " ".repeat(relation_indent),
        operator,
        format_pretty_expression(target, relation_indent)
    )
}

/// 输出 `any` 的多分支参数列表。
fn format_query_list(name: &str, queries: &[QueryExpr], indent: usize) -> String {
    let inner_indent = indent + 4;
    let mut output = format!("{name}(\n");
    for (index, query) in queries.iter().enumerate() {
        output.push_str(&" ".repeat(inner_indent));
        output.push_str(&format_pretty_expression(query, inner_indent));
        if index + 1 < queries.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str(&" ".repeat(indent));
    output.push(')');
    output
}

/// 输出单参数查询组合器。
fn format_unary(name: &str, query: &QueryExpr, indent: usize) -> String {
    let inner_indent = indent + 4;
    format!(
        "{name}(\n{}{}\n{})",
        " ".repeat(inner_indent),
        format_pretty_expression(query, inner_indent),
        " ".repeat(indent)
    )
}

/// 输出一个完整属性谓词。
fn format_predicate(predicate: &PropertyPredicate) -> String {
    format!(
        "{} {} {}",
        predicate.attribute,
        predicate.operator,
        format_value(&predicate.value)
    )
}

/// 输出谓词右值。
fn format_value(value: &PredicateValue) -> String {
    match value {
        PredicateValue::Text(text) => quote_string(text),
        PredicateValue::Boolean(value) => value.to_string(),
        PredicateValue::Regex(regex) => format_regex(regex),
    }
}

/// 使用 JSON 规则稳定转义文本。
fn quote_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a Rust string to JSON cannot fail")
}

/// 恢复 AQL 正则分隔符并转义模式中的 `/`。
fn format_regex(regex: &RegexLiteral) -> String {
    let mut output = String::from("/");
    for character in regex.pattern.chars() {
        if character == '/' {
            output.push('\\');
        }
        output.push(character);
    }
    output.push('/');
    if regex.case_insensitive {
        output.push('i');
    }
    output
}
