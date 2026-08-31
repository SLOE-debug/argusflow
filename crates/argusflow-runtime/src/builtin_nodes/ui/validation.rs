use std::collections::HashSet;

use argusflow_core::{
    AqlQuery, BackendKind, FieldProjectionSource, KeyboardKey, TargetLocator, TargetScope,
    TargetWaitMode, TargetWaitPolicy, UiExecutionPolicy, UiOperation, UiPostcondition,
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
    validate_extract(operation, context, &mut issues);
    validate_wait_policy(target, execution, context, &mut issues);
    validate_postcondition_wait(execution, context, &mut issues);
    validate_postcondition(operation, execution, context, &mut issues);
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
        // Vision 只提供文字事实；物理点击还必须由 SendInput 提交。
        let supported = matches!(operation, UiOperation::GetText { .. })
            || matches!(operation, UiOperation::Click { .. })
                && target.backend_policy.allows(BackendKind::SendInput)
            || matches!(operation, UiOperation::Extract { fields, .. } if fields.iter().all(
                |field| matches!(
                    field.source,
                    FieldProjectionSource::Text | FieldProjectionSource::Name
                )
            ));
        if !supported {
            issues.push(context.issue(
                ValidationIssueCode::InvalidBackendPolicy,
                "OCR AQL 仅支持读取文字、文字字段提取，以及配合键鼠输入的点击",
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
            format!("应用资源的后端策略必须允许 {application_backend:?}"),
        ));
    }
    if matches!(&target.scope, TargetScope::Browser { .. })
        && !target.backend_policy.allows(BackendKind::BrowserCdp)
    {
        issues.push(context.issue(
            ValidationIssueCode::InvalidBackendPolicy,
            "浏览器资源的后端策略必须允许 browser_cdp",
        ));
    }
    if matches!(operation, UiOperation::CollectLinks { .. })
        && !target.backend_policy.allows(BackendKind::BrowserCdp)
    {
        issues.push(context.issue(
            ValidationIssueCode::InvalidBackendPolicy,
            "批量链接读取的后端策略必须允许 browser_cdp",
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
                "按键和物理文本输入的后端策略必须允许 send_input",
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

/// 校验 Extract 字段集合及原生属性名称。
fn validate_extract(
    operation: &UiOperation,
    context: &NodeValidationContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    let UiOperation::Extract { fields, .. } = operation else {
        return;
    };
    let mut names = HashSet::new();
    if fields.is_empty() {
        issues.push(context.issue(
            ValidationIssueCode::InvalidExtract,
            "Extract 至少需要一个字段投影",
        ));
    }
    for field in fields {
        if field.name.trim().is_empty() || !names.insert(field.name.as_str()) {
            issues.push(context.issue(
                ValidationIssueCode::InvalidExtract,
                "Extract 字段名称必须非空且唯一",
            ));
        }
        if matches!(
            &field.source,
            FieldProjectionSource::Property { name }
                | FieldProjectionSource::Attribute { name }
                if name.trim().is_empty()
        ) {
            issues.push(context.issue(
                ValidationIssueCode::InvalidExtract,
                "Extract 属性名称不能为空",
            ));
        }
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
            "关闭目标等待时，超时和轮询间隔必须为 0",
        )),
        TargetWaitPolicy {
            mode: TargetWaitMode::Bounded,
            timeout_ms,
            poll_interval_ms,
        } if !(1..=600_000).contains(&timeout_ms) || !(1..=60_000).contains(&poll_interval_ms) => {
            issues.push(context.issue(
                ValidationIssueCode::InvalidTargetWaitPolicy,
                "目标等待超时必须在 1 到 600000 毫秒之间，轮询间隔必须在 1 到 60000 毫秒之间",
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

/// 校验视觉后置条件只附着在需要确认结果的物理输入动作上。
fn validate_postcondition(
    operation: &UiOperation,
    execution: &UiExecutionPolicy,
    context: &NodeValidationContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    let Some(postcondition) = &execution.postcondition else {
        return;
    };
    match postcondition {
        UiPostcondition::MatchAdded {
            query,
            stable_context,
        }
        | UiPostcondition::MatchRemoved {
            query,
            stable_context,
        } => {
            if !is_input_operation(operation) {
                issues.push(context.issue(
                    ValidationIssueCode::InvalidNodeDefinition,
                    "视觉匹配变化后置条件只能用于按键或物理文本输入动作",
                ));
            }
            validate_aql_query(query, "视觉匹配变化", context, issues);
            if stable_context.is_empty() {
                issues.push(context.issue(
                    ValidationIssueCode::InvalidNodeDefinition,
                    "视觉匹配变化后置条件必须至少绑定一条稳定上下文查询",
                ));
            }
            for query in stable_context {
                validate_aql_query(query, "视觉稳定上下文", context, issues);
            }
        }
        UiPostcondition::MatchPresent { query } => {
            if !matches!(operation, UiOperation::Click { .. }) && !is_input_operation(operation) {
                issues.push(context.issue(
                    ValidationIssueCode::InvalidNodeDefinition,
                    "视觉匹配存在后置条件只能用于点击、按键或物理文本输入动作",
                ));
            }
            validate_aql_query(query, "视觉匹配存在", context, issues);
        }
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

/// 校验发送后观察预算，确保后置条件不会退化成无界等待或无等待。
fn validate_postcondition_wait(
    execution: &UiExecutionPolicy,
    context: &NodeValidationContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    if execution.postcondition.is_none() {
        return;
    }
    match execution.postcondition_wait {
        TargetWaitPolicy {
            mode: TargetWaitMode::Bounded,
            timeout_ms,
            poll_interval_ms,
        } if (1..=600_000).contains(&timeout_ms) && (1..=60_000).contains(&poll_interval_ms) => {}
        _ => issues.push(context.issue(
            ValidationIssueCode::InvalidTargetWaitPolicy,
            "视觉后置条件必须配置 1 到 600000 毫秒的观察超时和 1 到 60000 毫秒的轮询间隔",
        )),
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
