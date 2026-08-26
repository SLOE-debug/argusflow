//! UIA 语义候选的有序去重、动作适配、唯一性解析与稳定错误映射。

use std::collections::HashSet;

use argusflow_core::{AutomationError, BackendKind};
use windows::Win32::UI::Accessibility::IUIAutomationElement;

use super::{
    action::{UiaActionStrategy, required_capability, resolve_action_strategy},
    error::UiaError,
    plan::{TargetResolutionFailure, UiaActionPlan, UiaResultLimit},
};

/// 只在 worker thread 生命周期内存在的 COM 元素及稳定去重键。
pub(super) struct ResolvedElement {
    /// UIA COM 元素，不允许穿过 runtime channel。
    pub(super) element: IUIAutomationElement,
    /// provider 返回的 UIA runtime id。
    pub(super) runtime_id: Vec<i32>,
}

/// 使用 runtime id 保持首次出现顺序的候选集合。
pub(super) struct ResolvedElementSet {
    /// 按 provider/遍历顺序保留的唯一元素。
    elements: Vec<ResolvedElement>,
    /// O(1) 平均复杂度的 runtime id 成员集合。
    runtime_ids: RuntimeIdIndex,
}

impl ResolvedElementSet {
    /// 创建空的有序唯一候选集合。
    pub(super) fn new() -> Self {
        Self {
            elements: Vec::new(),
            runtime_ids: RuntimeIdIndex::new(),
        }
    }

    /// 按预计候选数量创建集合，减少批量进程查询时的扩容。
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            elements: Vec::with_capacity(capacity),
            runtime_ids: RuntimeIdIndex::with_capacity(capacity),
        }
    }

    /// 插入尚未出现的 runtime id，并保留第一次出现的元素。
    pub(super) fn insert(&mut self, element: ResolvedElement) -> bool {
        if !self.runtime_ids.insert(&element.runtime_id) {
            return false;
        }
        self.elements.push(element);
        true
    }

    /// 批量插入候选，并在达到唯一结果上限后停止。
    pub(super) fn extend_until(
        &mut self,
        elements: impl IntoIterator<Item = ResolvedElement>,
        result_limit: UiaResultLimit,
    ) {
        for element in elements {
            self.insert(element);
            if result_limit.is_reached(self.len()) {
                break;
            }
        }
    }

    /// 返回当前唯一候选数。
    pub(super) fn len(&self) -> usize {
        self.elements.len()
    }

    /// 判断当前是否尚未收集任何唯一候选。
    pub(super) fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// 消费集合并返回有序元素。
    pub(super) fn into_vec(self) -> Vec<ResolvedElement> {
        self.elements
    }
}

/// 与 COM 元素所有权分离的 runtime id 成员索引。
struct RuntimeIdIndex {
    /// 完整 runtime id 数组必须整体参与相等性与哈希计算。
    ids: HashSet<Vec<i32>>,
}

impl RuntimeIdIndex {
    /// 创建空索引。
    fn new() -> Self {
        Self {
            ids: HashSet::new(),
        }
    }

    /// 创建指定容量的索引。
    fn with_capacity(capacity: usize) -> Self {
        Self {
            ids: HashSet::with_capacity(capacity),
        }
    }

    /// 插入完整 runtime id，并返回它是否首次出现。
    fn insert(&mut self, runtime_id: &[i32]) -> bool {
        self.ids.insert(runtime_id.to_vec())
    }
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

#[cfg(test)]
mod tests {
    use argusflow_core::{ActionCapability, AutomationError, BackendKind};

    use super::{RuntimeIdIndex, resolution_error};
    use crate::uia::plan::TargetResolutionFailure;

    /// 完整 runtime id 使用 HashSet 做常数时间成员判断，不能只比较前缀。
    #[test]
    fn runtime_id_index_uses_complete_identifier() {
        let mut index = RuntimeIdIndex::new();

        assert!(index.insert(&[1, 2]));
        assert!(!index.insert(&[1, 2]));
        assert!(index.insert(&[1, 3]));
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
