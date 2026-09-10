//! 在已冻结快照上执行选择器，所有后端共享比较及排序语义。
use super::{NodeKind, QueryTree, matcher};
use crate::{BoundQuery, Boundary, Expr, Relation, Role};
use argusflow_core::{Failure, FailureKind, Operation};
use std::collections::BTreeSet;

/// 求值并返回有序节点索引；不会因预算返回截断成功。
pub fn evaluate<T>(
    query: &BoundQuery,
    tree: &QueryTree<T>,
    operation: &Operation,
) -> Result<Vec<usize>, Failure> {
    match evaluate_step(query, tree, operation)? {
        QueryProgress::Complete(result) => Ok(result),
        QueryProgress::Boundary { .. } => Err(Failure::new(
            FailureKind::Unsupported,
            "aql_boundary",
            "来源没有所请求的文档边界",
        )),
    }
}
/// 浏览器可按需装载唯一宿主的文档边界，不遍历无关 frame。
#[derive(Debug)]
pub enum QueryProgress {
    /// 查询完成，索引按来源顺序排列。
    Complete(Vec<usize>),
    /// 此宿主的边界需要后端提供，再对同一快照继续求值。
    Boundary {
        /// 已唯一定位的宿主索引。
        host: usize,
        /// 请求的文档边界。
        boundary: Boundary,
    },
}
enum StepError {
    Failure(Failure),
    Boundary(usize, Boundary),
}
impl From<Failure> for StepError {
    fn from(value: Failure) -> Self {
        Self::Failure(value)
    }
}
/// 对当前树求值，缺少的显式边界交给适配器按需装载。
pub fn evaluate_step<T>(
    query: &BoundQuery,
    tree: &QueryTree<T>,
    operation: &Operation,
) -> Result<QueryProgress, Failure> {
    let candidates = tree.candidates(None, true, operation)?;
    let result = match eval(query.expression(), tree, &candidates, operation) {
        Ok(result) => result,
        Err(StepError::Failure(failure)) => return Err(failure),
        Err(StepError::Boundary(host, boundary)) => {
            return Ok(QueryProgress::Boundary { host, boundary });
        }
    };
    if result.len() > tree.max_results() {
        return Err(Failure::new(
            FailureKind::ResourceLimit,
            "aql_results",
            "查询结果超过预算",
        ));
    }
    if result
        .iter()
        .any(|index| tree.node(*index).and_then(|n| n.target()).is_none())
    {
        return Err(Failure::new(
            FailureKind::InvalidInput,
            "aql_results",
            "文档边界不能作为定位结果",
        ));
    }
    operation.check("aql_complete")?;
    Ok(QueryProgress::Complete(result))
}
fn eval<T>(
    expression: &Expr,
    tree: &QueryTree<T>,
    candidates: &[usize],
    operation: &Operation,
) -> Result<Vec<usize>, StepError> {
    operation.check("aql_evaluate")?;
    match expression {
        Expr::Match { role, condition } => {
            let mut matches = Vec::new();
            for &index in candidates {
                operation.check("aql_filter")?;
                if let Some(node) = tree.node(index)
                    && matches!(node.kind(), NodeKind::Element(actual) if *role == Role::Element || *role == actual)
                    && condition
                        .as_ref()
                        .is_none_or(|c| matcher::matches(c, node.attributes()) == Some(true))
                {
                    matches.push(index);
                }
            }
            Ok(matches)
        }
        Expr::Css(selector) => Ok(candidates
            .iter()
            .copied()
            .filter(|i| tree.node(*i).is_some_and(|n| n.css_matches(selector)))
            .collect()),
        Expr::Nth { query, index } => Ok(eval(query, tree, candidates, operation)?
            .get(index.get() - 1)
            .copied()
            .into_iter()
            .collect()),
        Expr::Enter { host, boundary } => {
            let hosts = eval(host, tree, candidates, operation)?;
            if hosts.len() != 1 {
                return Err(Failure::new(
                    if hosts.is_empty() {
                        FailureKind::NotFound
                    } else {
                        FailureKind::Ambiguous
                    },
                    "aql_boundary",
                    "文档边界宿主必须唯一",
                )
                .into());
            }
            tree.boundary(hosts[0], *boundary)
                .map(|root| vec![root])
                .ok_or(StepError::Boundary(hosts[0], *boundary))
        }
        Expr::Relation {
            left,
            right,
            relation,
        } => {
            let roots = eval(left, tree, candidates, operation)?;
            let mut seen = BTreeSet::new();
            for root in roots {
                for index in
                    tree.candidates(Some(root), *relation == Relation::Descendant, operation)?
                {
                    seen.insert(index);
                }
            }
            let nested = tree.ordered(seen, operation)?;
            eval(right, tree, &nested, operation)
        }
    }
}
