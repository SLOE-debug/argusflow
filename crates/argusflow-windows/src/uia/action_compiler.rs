//! UIA 查询最终目标角色与动作 pattern 的联合能力证明。

use argusflow_core::AutomationAction;
use argusflow_query::SupportLevel;
use thiserror::Error;

use super::{
    native::{UiaControlType, UiaRoleConstraint},
    plan::{UiaActionPlan, UiaActionSupport, UiaPlanExpr, UiaPreparedPlan, UiaQueryPlan},
};

/// UIA 无法为查询最终角色保持动作语义时返回的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UiaActionCompileError {
    /// 至少一个可能成为 `any` 首个非空结果的角色不支持当前动作策略。
    #[error("Windows UI Automation cannot preserve the action semantics for target role {role:?}")]
    UnsupportedTargetRole {
        /// 无法通过当前动作 pattern 保持语义的目标角色。
        role: UiaRoleConstraint,
    },
}

/// 把动作与已编译查询合并成不可在 execute 阶段重新规划的 UIA 计划。
pub fn compile_uia_action(
    action: &AutomationAction,
    query: UiaQueryPlan,
) -> Result<UiaPreparedPlan, UiaActionCompileError> {
    let action_plan = match action {
        AutomationAction::Click { .. } => UiaActionPlan::Invoke,
        AutomationAction::SetValue { value, .. } => UiaActionPlan::SetValue {
            value: value.clone(),
        },
    };
    let mut roles = Vec::new();
    collect_target_roles(&query.expression, &mut roles);
    let action_support =
        roles
            .into_iter()
            .try_fold(UiaActionSupport::Native, |combined, role| {
                let support = role_action_support(role, &action_plan);
                if matches!(support, UiaActionSupport::Unsupported) {
                    Err(UiaActionCompileError::UnsupportedTargetRole { role })
                } else {
                    Ok(combine_action_support(combined, support))
                }
            })?;
    let mut capability = query.capability;
    capability.level = combine_query_action_support(capability.level, action_support);

    Ok(UiaPreparedPlan {
        query,
        action: action_plan,
        action_support,
        capability,
    })
}

/// 只收集关系表达式最终返回的角色；祖先和父节点不会成为动作目标。
fn collect_target_roles(expression: &UiaPlanExpr, roles: &mut Vec<UiaRoleConstraint>) {
    match expression {
        UiaPlanExpr::Match(matcher) => {
            if !roles.contains(&matcher.role) {
                roles.push(matcher.role);
            }
        }
        UiaPlanExpr::Descendant { target, .. } | UiaPlanExpr::Child { target, .. } => {
            collect_target_roles(target, roles);
        }
        UiaPlanExpr::Any(branches) => {
            for branch in branches {
                collect_target_roles(branch, roles);
            }
        }
        UiaPlanExpr::First(query) | UiaPlanExpr::Nth { query, .. } => {
            collect_target_roles(query, roles);
        }
    }
}

/// 根据 UIA control pattern 规范判断角色能否保持当前动作语义。
const fn role_action_support(role: UiaRoleConstraint, action: &UiaActionPlan) -> UiaActionSupport {
    match (action, role) {
        (
            UiaActionPlan::Invoke,
            UiaRoleConstraint::ControlType(UiaControlType::Button | UiaControlType::Hyperlink),
        ) => UiaActionSupport::Native,
        (UiaActionPlan::Invoke, UiaRoleConstraint::ControlType(UiaControlType::MenuItem)) => {
            UiaActionSupport::RequiresRuntimePatternCheck
        }
        (UiaActionPlan::SetValue { .. }, UiaRoleConstraint::ControlType(UiaControlType::Edit)) => {
            UiaActionSupport::Native
        }
        (
            UiaActionPlan::SetValue { .. },
            UiaRoleConstraint::ControlType(UiaControlType::ComboBox),
        ) => UiaActionSupport::RequiresRuntimePatternCheck,
        _ => UiaActionSupport::Unsupported,
    }
}

/// 返回两个动作支持等级中约束更弱的一项。
const fn combine_action_support(
    left: UiaActionSupport,
    right: UiaActionSupport,
) -> UiaActionSupport {
    match (left, right) {
        (UiaActionSupport::Unsupported, _) | (_, UiaActionSupport::Unsupported) => {
            UiaActionSupport::Unsupported
        }
        (UiaActionSupport::RequiresRuntimePatternCheck, _)
        | (_, UiaActionSupport::RequiresRuntimePatternCheck) => {
            UiaActionSupport::RequiresRuntimePatternCheck
        }
        _ => UiaActionSupport::Native,
    }
}

/// 把实例 pattern 复验反映为 Hybrid，避免 Explain 把运行时不确定性报告成 Native。
const fn combine_query_action_support(
    query: SupportLevel,
    action: UiaActionSupport,
) -> SupportLevel {
    let action_level = match action {
        UiaActionSupport::Native => SupportLevel::Native,
        UiaActionSupport::RequiresRuntimePatternCheck => SupportLevel::Hybrid,
        UiaActionSupport::Unsupported => SupportLevel::Unsupported,
    };
    if query.rank() >= action_level.rank() {
        query
    } else {
        action_level
    }
}
