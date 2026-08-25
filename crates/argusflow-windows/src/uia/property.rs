//! cached UIA property 的类型转换与 residual 求值。

use windows::{
    Win32::{System::Variant::VARIANT, UI::Accessibility::IUIAutomationElement},
    core::BSTR,
};

use super::{
    condition::property_id,
    error::{UiaError, UiaOperation},
    native::{UiaProperty, UiaPropertyProjection, UiaResidualMatcher, UiaResidualPredicate},
};

/// 对单个候选执行全部本地 residual 谓词。
pub(crate) fn matches_residual(
    element: &IUIAutomationElement,
    predicates: &[UiaResidualPredicate],
) -> Result<bool, UiaError> {
    for predicate in predicates {
        let value = read_cached_property(element, predicate.projection)?;
        if !matches_value(&value, &predicate.matcher)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// 读取 compiler 明确加入 CacheRequest 的属性。
fn read_cached_property(
    element: &IUIAutomationElement,
    projection: UiaPropertyProjection,
) -> Result<String, UiaError> {
    let property = projection.property();
    // SAFETY: 每个 getter 只读取 CacheRequest 已声明的属性，element 留在 UIA worker。
    match property {
        UiaProperty::Name => unsafe { element.CachedName() }
            .map(bstr_value)
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source)),
        // SAFETY: AutomationId 已加入当前 element 的 CacheRequest，element 留在 worker。
        UiaProperty::AutomationId => unsafe { element.CachedAutomationId() }
            .map(bstr_value)
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source)),
        // SAFETY: ClassName 已加入当前 element 的 CacheRequest，element 留在 worker。
        UiaProperty::ClassName => unsafe { element.CachedClassName() }
            .map(bstr_value)
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source)),
        UiaProperty::Value => {
            // SAFETY: property 已由 compiler 加入 CacheRequest，element 未跨 apartment。
            let variant = unsafe { element.GetCachedPropertyValue(property_id(property)) }
                .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?;
            variant_value(property, &variant)
        }
        UiaProperty::IsEnabled
        | UiaProperty::IsOffscreen
        | UiaProperty::HasKeyboardFocus
        | UiaProperty::ToggleState
        | UiaProperty::IsSelected
        | UiaProperty::IsDialog => Err(UiaError::PropertyTypeMismatch { property }),
    }
}

/// 把拥有所有权的 BSTR 转换为 Rust 字符串。
fn bstr_value(value: BSTR) -> String {
    value.to_string()
}

/// 按 compiler 证明的 property 类型转换 VARIANT。
fn variant_value(property: UiaProperty, value: &VARIANT) -> Result<String, UiaError> {
    match property {
        UiaProperty::Value => BSTR::try_from(value)
            .map(bstr_value)
            .map_err(|_| UiaError::PropertyTypeMismatch { property }),
        _ => Err(UiaError::PropertyTypeMismatch { property }),
    }
}

/// 对缓存值应用已冻结的本地 matcher。
fn matches_value(value: &str, matcher: &UiaResidualMatcher) -> Result<bool, UiaError> {
    match matcher {
        UiaResidualMatcher::Contains(expected) => Ok(value.contains(expected)),
        UiaResidualMatcher::StartsWith(expected) => Ok(value.starts_with(expected)),
        UiaResidualMatcher::EndsWith(expected) => Ok(value.ends_with(expected)),
        UiaResidualMatcher::Regex(regex) => Ok(regex.is_match(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::matches_value;
    use crate::uia::native::UiaResidualMatcher;

    #[test]
    fn residual_string_operators_keep_declared_semantics() {
        let value = "Find Next";

        assert!(
            matches_value(value, &UiaResidualMatcher::Contains("Next".to_owned()))
                .expect("contains matcher should execute")
        );
        assert!(
            matches_value(value, &UiaResidualMatcher::StartsWith("Find".to_owned()))
                .expect("starts_with matcher should execute")
        );
        assert!(
            matches_value(value, &UiaResidualMatcher::EndsWith("Next".to_owned()))
                .expect("ends_with matcher should execute")
        );
    }

    #[test]
    fn residual_regex_honors_case_insensitive_flag() {
        let value = "CLOSE";
        let matcher = UiaResidualMatcher::Regex(
            crate::uia::native::UiaResidualRegex::new("close", true)
                .expect("test regex should compile"),
        );

        assert!(matches_value(value, &matcher).expect("regex matcher should execute"));
    }
}
