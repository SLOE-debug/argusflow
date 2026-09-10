//! 输入前在 MTA 内重新核验可编辑性和实际焦点。
use crate::{
    WindowsError,
    platform::{failure, pattern_failure},
};
use argusflow_core::{FailureKind, Operation};
use windows::Win32::UI::Accessibility::*;

pub(in crate::uia) fn focus(
    element: &IUIAutomationElement,
    operation: &Operation,
) -> Result<(), WindowsError> {
    // SAFETY: 元素、Pattern 及属性调用均位于拥有它们的 MTA 线程。
    unsafe {
        let value: IUIAutomationValuePattern = element
            .GetCurrentPatternAs(UIA_ValuePatternId)
            .map_err(|e| pattern_failure("aql_editable", e))?;
        if value
            .CurrentIsReadOnly()
            .map_err(|e| failure("aql_readonly", e))?
            .as_bool()
            || !element
                .CurrentIsEnabled()
                .map_err(|e| failure("aql_enabled", e))?
                .as_bool()
            || element
                .CurrentIsOffscreen()
                .map_err(|e| failure("aql_offscreen", e))?
                .as_bool()
        {
            return Err(WindowsError::new(
                FailureKind::Unsupported,
                "aql_focus",
                "UIA 目标只读、不可用或不可见",
            ));
        }
        operation.begin_effect("aql_focus")?;
        if !element
            .CurrentHasKeyboardFocus()
            .map_err(|e| failure("aql_focus_check", e))?
            .as_bool()
        {
            element.SetFocus().map_err(|e| failure("aql_focus", e))?;
        }
        if !element
            .CurrentHasKeyboardFocus()
            .map_err(|e| failure("aql_focus_check", e))?
            .as_bool()
        {
            return Err(WindowsError::new(
                FailureKind::StaleHandle,
                "aql_focus",
                "UIA 目标没有获得键盘焦点",
            ));
        }
        let text: IUIAutomationTextPattern = element
            .GetCurrentPatternAs(UIA_TextPatternId)
            .map_err(|e| pattern_failure("aql_selection", e))?;
        let ranges = text
            .GetSelection()
            .map_err(|e| failure("aql_selection", e))?;
        if ranges.Length().map_err(|e| failure("aql_selection", e))? != 1 {
            return Err(WindowsError::new(
                FailureKind::Unsupported,
                "aql_selection",
                "UIA 输入需要单一光标或选区",
            ));
        }
        let range = ranges
            .GetElement(0)
            .map_err(|e| failure("aql_selection", e))?;
        // 收起选区到末尾，保证文字插入不会替换已有选择。
        range
            .MoveEndpointByRange(
                TextPatternRangeEndpoint_Start,
                &range,
                TextPatternRangeEndpoint_End,
            )
            .map_err(|e| failure("aql_caret", e))?;
        range.Select().map_err(|e| failure("aql_caret", e))?;
    }
    operation.check("aql_focus_complete").map_err(Into::into)
}
