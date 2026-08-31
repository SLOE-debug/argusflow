use std::collections::HashSet;

use argusflow_core::{
    AqlQuery, BackendKind, KeyboardKey, TargetLocator, TargetScope, TargetWaitMode,
    TargetWaitPolicy, UiExecutionPolicy, UiOperation,
};
use argusflow_query::{parse_stored_query, query_parameter_names};

use crate::{NodeValidationContext, ValidationIssue, ValidationIssueCode};

/// 校验 UI 操作的后端、目标、输入负载和目标等待契约。
pub(super) fn validate_ui_node(
    operation: &UiOperation,
    execution: &UiExecutionPolicy,
    context: &NodeValidationContext<'_>,
) -> Vec<ValidationIssue> {
    let target = operation.target();
    let mut issues = Vec::new();
    validate_backend_policy(operation, context, &mut issues);
    validate_input_operation(operation, context, &mut issues);
    validate_wait_policy(target, execution, context, &mut issues);
    validate_locator(operation, context, &mut issues);
    issues
}

/// 校验资源作用域和操作类别要求的后端集合。
fn validate_backend_policy(
    operation: &UiOperation,
    context: &NodeValidationContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    let target = operation.target();
    // 显式 OCR 查询不需要同时开放 UIA；它拥有独立的动作能力边界。
    let explicitly_uses_ocr = matches!(target.locator, TargetLocator::Query { .. })
        && target.backend_policy.allow.contains(&BackendKind::OcrSmall)
        && !target.backend_policy.allows(BackendKind::WindowsUia);
    if explicitly_uses_ocr {
        // Vision 只负责物化文字目标；实际点击仍由 SendInput 提交。
        let supported = matches!(operation, UiOperation::Click { .. })
            && target.backend_policy.allows(BackendKind::SendInput);
        if !supported {
            issues.push(context.issue(
                ValidationIssueCode::InvalidBackendPolicy,
                "画面文字识别只能用于点击可见文字。要读取界面内容，请使用“检查界面”节点",
            ));
        }
        return;
    }
    let application_backend = if is_input_operation(operation) {
        BackendKind::SendInput
    } else {
        BackendKind::WindowsUia
    };
    if matches!(&target.scope, TargetScope::Application { .. })
        && !target.backend_policy.allows(application_backend)
    {
        issues.push(context.issue(
            ValidationIssueCode::InvalidBackendPolicy,
            format!("当前操作方式无法用于所选应用，请允许 {application_backend:?} 或更换操作方式"),
        ));
    }
    if matches!(&target.scope, TargetScope::Browser { .. })
        && !target.backend_policy.allows(BackendKind::BrowserCdp)
    {
        issues.push(context.issue(
            ValidationIssueCode::InvalidBackendPolicy,
            "当前操作方式无法用于所选浏览器，请改用浏览器自动化",
        ));
    }
}

/// 校验键盘动作只使用当前焦点和 SendInput，并验证组合键字段。
fn validate_input_operation(
    operation: &UiOperation,
    context: &NodeValidationContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    if is_input_operation(operation) {
        let target = operation.target();
        if matches!(&target.scope, TargetScope::Browser { .. }) {
            issues.push(context.issue(
                ValidationIssueCode::InvalidNodeDefinition,
                "按键和物理文本输入不能绑定浏览器资源",
            ));
        }
        if !matches!(&target.locator, TargetLocator::Focused) {
            issues.push(context.issue(
                ValidationIssueCode::InvalidNodeDefinition,
                "按键和物理文本输入必须使用当前焦点目标",
            ));
        }
        if !target.backend_policy.allows(BackendKind::SendInput) {
            issues.push(context.issue(
                ValidationIssueCode::InvalidBackendPolicy,
                "按键和文字输入需要使用“模拟键盘输入”方式",
            ));
        }
    }
    let UiOperation::PressKey { chord, .. } = operation else {
        return;
    };
    let unique_modifiers = chord.modifiers.iter().collect::<HashSet<_>>();
    if unique_modifiers.len() != chord.modifiers.len() {
        issues.push(context.issue(
            ValidationIssueCode::InvalidNodeDefinition,
            "组合键不能包含重复修饰键",
        ));
    }
    if matches!(
        &chord.key,
        KeyboardKey::Character { value }
            if value.len() != 1 || !value.as_bytes()[0].is_ascii_alphanumeric()
    ) {
        issues.push(context.issue(
            ValidationIssueCode::InvalidNodeDefinition,
            "组合键字符必须是单个 ASCII 字母或数字",
        ));
    }
}

/// 校验当前定位类别是否允许配置目标等待。
fn validate_wait_policy(
    target: &argusflow_core::AutomationTarget,
    execution: &UiExecutionPolicy,
    context: &NodeValidationContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    match execution.target_wait {
        TargetWaitPolicy {
            mode: TargetWaitMode::None,
            timeout_ms: 0,
            poll_interval_ms: 0,
        } => {}
        TargetWaitPolicy {
            mode: TargetWaitMode::None,
            ..
        } => issues.push(context.issue(
            ValidationIssueCode::InvalidTargetWaitPolicy,
            "已关闭自动等待，请把最长等待和检查间隔都设为 0",
        )),
        TargetWaitPolicy {
            mode: TargetWaitMode::Bounded,
            timeout_ms,
            poll_interval_ms,
        } if !(1..=600_000).contains(&timeout_ms) || !(1..=60_000).contains(&poll_interval_ms) => {
            issues.push(context.issue(
                ValidationIssueCode::InvalidTargetWaitPolicy,
                "最长等待应为 1 到 600000 毫秒，检查间隔应为 1 到 60000 毫秒",
            ));
        }
        TargetWaitPolicy {
            mode: TargetWaitMode::Bounded,
            ..
        } if matches!(
            &target.locator,
            TargetLocator::Coordinate { .. } | TargetLocator::Focused
        ) =>
        {
            issues.push(context.issue(
                ValidationIssueCode::InvalidTargetWaitPolicy,
                "坐标或当前焦点目标没有元素就绪语义，不能启用目标等待",
            ));
        }
        TargetWaitPolicy {
            mode: TargetWaitMode::Bounded,
            ..
        } => {}
    }
}

/// 校验后置条件使用的 AQL 语法以及源码和参数绑定集合的一致性。
fn validate_aql_query(
    query: &AqlQuery,
    label: &str,
    context: &NodeValidationContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    match parse_stored_query(query) {
        Err(error) => {
            let help = error
                .help
                .as_deref()
                .map(|help| format!("；建议：{help}"))
                .unwrap_or_default();
            issues.push(context.issue(
                ValidationIssueCode::InvalidAqlQuery,
                format!("{label} AQL 查询无效：{error}{help}"),
            ));
        }
        Ok(parsed) => {
            let referenced = query_parameter_names(&parsed);
            let bound = query.bindings.keys().cloned().collect();
            if referenced != bound {
                issues.push(context.issue(
                    ValidationIssueCode::InvalidAqlQuery,
                    format!(
                        "{label} AQL 参数绑定与源码不一致：引用 {referenced:?}，绑定 {bound:?}"
                    ),
                ));
            }
        }
    }
}

/// 校验 AQL、视觉文字和当前焦点定位的专属约束。
fn validate_locator(
    operation: &UiOperation,
    context: &NodeValidationContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    match &operation.target().locator {
        TargetLocator::Query { query } => {
            validate_aql_query(query, "目标", context, issues);
        }
        TargetLocator::Focused if !is_input_operation(operation) => {
            issues.push(context.issue(
                ValidationIssueCode::InvalidNodeDefinition,
                "只有按键和物理文本输入可以使用当前焦点目标",
            ));
        }
        TargetLocator::Coordinate { .. } | TargetLocator::Focused => {}
    }
}

/// 判断操作是否依赖应用当前键盘焦点。
const fn is_input_operation(operation: &UiOperation) -> bool {
    matches!(
        operation,
        UiOperation::PressKey { .. } | UiOperation::TypeText { .. }
    )
}
