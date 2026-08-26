//! UIA 语义候选的动作适配、唯一性解析与稳定错误映射。

use argusflow_core::{AutomationError, BackendKind};
use windows::Win32::UI::Accessibility::IUIAutomationElement;

use super::{
    action::{UiaActionStrategy, required_capability, resolve_action_strategy},
    error::UiaError,
    plan::{TargetResolutionFailure, UiaActionPlan},
};

/// 只在 worker thread 生命周期内存在的 COM 元素及稳定去重键。
pub(super) struct ResolvedElement {
    /// UIA COM 元素，不允许穿过 runtime channel。
    pub(super) element: IUIAutomationElement,
    /// provider 返回的 UIA runtime id。
    pub(super) runtime_id: Vec<i32>,
}

/// 只在 worker apartment 内存在的目标与精确动作策略。
pub(super) struct ResolvedActionTarget {
    /// 已通过 matcher 的语义元素。
    pub(super) element: ResolvedElement,
    /// suitability filter 冻结的 pattern 策略。
    pub(super) strategy: UiaActionStrategy,
}

/// action suitability 既可能遇到 provider 错误，也可能得到稳定解析失败。
pub(super) enum TargetSelectionError {
    /// UIA provider 或元素生命周期错误。
    Uia(UiaError),
    /// 不依赖 HRESULT 的目标解析结论。
    Resolution(TargetResolutionFailure),
}

/// 在唯一性解析前过滤不具备当前动作能力的语义候选。
pub(super) fn resolve_action_target(
    candidates: Vec<ResolvedElement>,
    action: &UiaActionPlan,
) -> Result<ResolvedActionTarget, TargetSelectionError> {
    if candidates.is_empty() {
        return Err(TargetSelectionError::Resolution(
            TargetResolutionFailure::NotFound,
        ));
    }
    let semantic_matches = candidates.len();
    let mut suitable = Vec::new();
    for element in candidates {
        if let Some(strategy) =
            resolve_action_strategy(&element.element, action).map_err(TargetSelectionError::Uia)?
        {
            suitable.push(ResolvedActionTarget { element, strategy });
        }
    }
    match suitable.len() {
        0 => Err(TargetSelectionError::Resolution(
            TargetResolutionFailure::ActionUnsupported {
                semantic_matches,
                required: required_capability(action),
            },
        )),
        1 => suitable
            .into_iter()
            .next()
            .ok_or(TargetSelectionError::Resolution(
                TargetResolutionFailure::NotFound,
            )),
        matches => Err(TargetSelectionError::Resolution(
            TargetResolutionFailure::Ambiguous { matches },
        )),
    }
}

/// 把强类型目标解析失败转换为不会错误触发 selector fallback 的公共错误。
pub(super) fn resolution_error(failure: TargetResolutionFailure, query: &str) -> AutomationError {
    match failure {
        TargetResolutionFailure::NotFound => AutomationError::TargetNotFound {
            query: query.to_owned(),
        },
        TargetResolutionFailure::Ambiguous { matches } => AutomationError::AmbiguousTarget {
            query: query.to_owned(),
            matches,
        },
        TargetResolutionFailure::ActionUnsupported {
            semantic_matches,
            required,
        } => AutomationError::ActionUnsupported {
            backend: BackendKind::WindowsUia,
            query: query.to_owned(),
            semantic_matches,
            required,
        },
    }
}

/// 按 UIA runtime id 去重，同时保留首次出现顺序。
pub(super) fn append_unique(
    destination: &mut Vec<ResolvedElement>,
    elements: impl IntoIterator<Item = ResolvedElement>,
) {
    for element in elements {
        if !contains_runtime_id(
            destination
                .iter()
                .map(|existing| existing.runtime_id.as_slice()),
            &element.runtime_id,
        ) {
            destination.push(element);
        }
    }
}

/// 判断当前结果集合是否已包含相同 runtime id。
fn contains_runtime_id<'id>(
    existing: impl IntoIterator<Item = &'id [i32]>,
    candidate: &[i32],
) -> bool {
    existing
        .into_iter()
        .any(|runtime_id| runtime_id == candidate)
}

#[cfg(test)]
mod tests {
    use argusflow_core::{ActionCapability, AutomationError, BackendKind};

    use super::{contains_runtime_id, resolution_error};
    use crate::uia::plan::TargetResolutionFailure;

    /// 验证与 COM 元素无关的 runtime id 顺序去重规则。
    #[test]
    fn runtime_id_membership_uses_the_complete_identifier() {
        let ids = [vec![1, 2], vec![3, 4]];

        assert!(contains_runtime_id(ids.iter().map(Vec::as_slice), &[1, 2]));
        assert!(!contains_runtime_id(ids.iter().map(Vec::as_slice), &[1, 3]));
    }

    /// 动作能力不足必须保留独立错误种类，不能伪装成 selector miss 触发 fallback。
    #[test]
    fn action_unsupported_is_not_mapped_to_target_not_found() {
        let error = resolution_error(
            TargetResolutionFailure::ActionUnsupported {
                semantic_matches: 2,
                required: ActionCapability::WriteValue,
            },
            "uia[role=edit]",
        );

        assert_eq!(
            error,
            AutomationError::ActionUnsupported {
                backend: BackendKind::WindowsUia,
                query: "uia[role=edit]".to_owned(),
                semantic_matches: 2,
                required: ActionCapability::WriteValue,
            }
        );
    }
}
