//! 确定性 AQL v3 selector synthesis；不调用 AI，不推测 Nth 或未观察到的祖先。

use crate::ResolutionBackend;
use argusflow_core::{
    AqlQuery, DomAttribute, ElementMatcher, ElementRole, ElementSemantics, InspectedEntity,
    MatchOperator, PredicateValue, PropertyPredicate, QueryExpr, ScreenPoint, SelectorAttribute,
    UiQuery, UiaAttribute,
};
use serde::{Deserialize, Serialize};

/// 每个候选来自已采集事实，坐标保持显式 fallback 而不冒充 AQL。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum RecordedSelector {
    /// 可直接写入现有 WorkflowDefinition target query 的 v3 源码。
    Aql(AqlQuery),
    /// 最低稳定性的物理屏幕坐标。
    Coordinate(ScreenPoint),
}

/// Selector 稳定性评分的可解释依据。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateBasis {
    /// UIA provider 指定的控件标识。
    AutomationId,
    /// DOM 测试标识。
    TestId,
    /// 不具有明显生成特征的 DOM id。
    StableId,
    /// 可读角色与名称。
    RoleName,
    /// 已观察到的稳定祖先与后代关系。
    StableAncestor,
    /// class 可能受构建或样式变化影响。
    Class,
    /// hash、数字后缀等动态特征降权。
    DynamicAttribute,
    /// OCR 名称的视觉回放候选。
    VisualText,
    /// 屏幕坐标依赖布局。
    Coordinate,
}

/// 按稳定性降序排列；评分不宣称 live tree 唯一性。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectorCandidate {
    /// AQL 或显式坐标。
    pub selector: RecordedSelector,
    /// 0..=100，纯确定性启发式。
    pub stability_score: u8,
    /// 评分依据。
    pub basis: CandidateBasis,
    /// 候选必须交给哪个解析/执行能力。
    pub backend: ResolutionBackend,
}

/// 从实体事实生成候选，所有字符串通过 AQL AST formatter 处理转义。
pub fn synthesize_selectors(
    entity: Option<&InspectedEntity>,
    backend: ResolutionBackend,
    point: Option<ScreenPoint>,
) -> Vec<SelectorCandidate> {
    let mut candidates = Vec::new();
    if let Some(entity) = entity {
        let semantics = &entity.semantics;
        if let Some(role) = semantics.role {
            for (attribute, value, basis, score) in attributes(semantics, backend) {
                push_query(
                    &mut candidates,
                    matcher(role, attribute, value),
                    basis,
                    score,
                    backend,
                );
            }
            if let Some(name) = semantics.name.as_deref().filter(|value| !value.is_empty()) {
                let target = matcher(role, SelectorAttribute::Name, name);
                push_query(
                    &mut candidates,
                    target.clone(),
                    if backend == ResolutionBackend::Vision {
                        CandidateBasis::VisualText
                    } else {
                        CandidateBasis::RoleName
                    },
                    if backend == ResolutionBackend::Vision {
                        55
                    } else {
                        80
                    },
                    backend,
                );
                for ancestor in &entity.ancestors {
                    let Some(role) = ancestor.role else {
                        continue;
                    };
                    let Some((attribute, value, _, score)) = attributes(ancestor, backend)
                        .into_iter()
                        .find(|(_, _, _, score)| *score >= 90)
                    else {
                        continue;
                    };
                    push_query(
                        &mut candidates,
                        QueryExpr::Descendant {
                            ancestor: Box::new(matcher(role, attribute, value)),
                            target: Box::new(target.clone()),
                        },
                        CandidateBasis::StableAncestor,
                        score.saturating_sub(5),
                        backend,
                    );
                    break;
                }
            }
        }
    }
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.stability_score));
    candidates.dedup_by(|a, b| a.selector == b.selector);
    if let Some(point) = point {
        candidates.push(SelectorCandidate {
            selector: RecordedSelector::Coordinate(point),
            stability_score: 5,
            basis: CandidateBasis::Coordinate,
            backend: ResolutionBackend::Coordinate,
        });
    }
    candidates
}

/// 只使用当前后端已有的 AQL 属性，不给 Vision 编造 UIA/DOM 条件。
fn attributes<'a>(
    semantics: &'a ElementSemantics,
    backend: ResolutionBackend,
) -> Vec<(SelectorAttribute, &'a str, CandidateBasis, u8)> {
    let mut output = Vec::new();
    let mut add = |attribute, value: Option<&'a str>, basis, score| {
        if let Some(value) = value.filter(|value| !value.is_empty() && value.len() <= 512) {
            let dynamic = looks_dynamic(value);
            output.push((
                attribute,
                value,
                if dynamic {
                    CandidateBasis::DynamicAttribute
                } else {
                    basis
                },
                if dynamic { 25 } else { score },
            ));
        }
    };
    match backend {
        ResolutionBackend::Uia => {
            add(
                SelectorAttribute::Uia(UiaAttribute::AutomationId),
                semantics.automation_id.as_deref(),
                CandidateBasis::AutomationId,
                98,
            );
            add(
                SelectorAttribute::Uia(UiaAttribute::ClassName),
                semantics.class_name.as_deref(),
                CandidateBasis::Class,
                40,
            );
        }
        ResolutionBackend::ManagedCdp => {
            add(
                SelectorAttribute::Dom(DomAttribute::TestId),
                semantics.test_id.as_deref(),
                CandidateBasis::TestId,
                98,
            );
            add(
                SelectorAttribute::Key,
                semantics.stable_id.as_deref(),
                CandidateBasis::StableId,
                93,
            );
            add(
                SelectorAttribute::Dom(DomAttribute::Class),
                semantics.class_name.as_deref(),
                CandidateBasis::Class,
                35,
            );
        }
        ResolutionBackend::Vision | ResolutionBackend::Coordinate => {}
    }
    output
}

/// 动态特征只影响评分，不删除用户可审查的候选。
fn looks_dynamic(value: &str) -> bool {
    value.starts_with(':')
        || value.contains("css-")
        || value.contains("sc-")
        || value
            .split(|ch: char| !ch.is_ascii_hexdigit())
            .any(|part| part.len() >= 8 && part.chars().any(|ch| ch.is_ascii_digit()))
        || value.chars().rev().take_while(char::is_ascii_digit).count() >= 3
}

fn matcher(role: ElementRole, attribute: SelectorAttribute, value: &str) -> QueryExpr {
    QueryExpr::Match {
        matcher: ElementMatcher {
            role,
            predicates: vec![PropertyPredicate {
                attribute,
                operator: MatchOperator::Equal,
                value: PredicateValue::Text(value.to_owned()),
            }],
        },
    }
}

fn push_query(
    candidates: &mut Vec<SelectorCandidate>,
    expression: QueryExpr,
    basis: CandidateBasis,
    stability_score: u8,
    backend: ResolutionBackend,
) {
    let source = argusflow_query::canonicalize_query(&UiQuery::new(expression));
    candidates.push(SelectorCandidate {
        selector: RecordedSelector::Aql(AqlQuery::v3(source)),
        stability_score,
        basis,
        backend,
    });
}
