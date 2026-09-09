//! 明确调用指定 UIA Pattern，不实现隐式输入回退。
use crate::WindowsError as Failure;
use crate::platform::{failure, pattern_failure};
use argusflow_core::{Operation, ScrollAxis};
use windows::{Win32::UI::Accessibility::*, core::BSTR};

/// 控件滚动单位；不同于鼠标滚轮步数。
#[derive(Debug, Clone, Copy)]
pub enum ScrollAmount {
    /// 小幅向后。
    SmallDecrement,
    /// 小幅向前。
    SmallIncrement,
    /// 大幅向后。
    LargeDecrement,
    /// 大幅向前。
    LargeIncrement,
}

/// SelectionItemPattern 的选择动作。
#[derive(Debug, Clone, Copy)]
pub enum SelectionAction {
    /// 替换当前选择。
    Select,
    /// 添加到当前选择。
    Add,
    /// 从当前选择移除。
    Remove,
}

/// 首版 UIA 可执行的封闭动作集合。
#[derive(Debug, Clone)]
pub enum UiaAction {
    /// InvokePattern 激活。
    Invoke,
    /// ValuePattern 设置完整值。
    SetValue(String),
    /// 请求控件键盘焦点。
    Focus,
    /// TogglePattern 切换一次状态。
    Toggle,
    /// SelectionItemPattern 操作。
    Selection(SelectionAction),
    /// 展开控件。
    Expand,
    /// 折叠控件。
    Collapse,
    /// ScrollPattern 滚动。
    Scroll {
        /// 滚动轴。
        axis: ScrollAxis,
        /// 控件滚动量。
        amount: ScrollAmount,
    },
    /// ScrollItemPattern 滚动到可见位置。
    ScrollIntoView,
}

pub(crate) fn execute(
    element: &IUIAutomationElement,
    action: UiaAction,
    operation: &Operation,
) -> Result<(), Failure> {
    // SAFETY: element 和所有 pattern 都只在拥有 COM apartment 的 worker 线程使用。
    unsafe {
        match action {
            UiaAction::Invoke => {
                let pattern: IUIAutomationInvokePattern = element
                    .GetCurrentPatternAs(UIA_InvokePatternId)
                    .map_err(|e| pattern_failure("invoke_pattern", e))?;
                operation.begin_effect("invoke")?;
                pattern.Invoke().map_err(|e| failure("invoke", e))?;
            }
            UiaAction::SetValue(value) => {
                let pattern: IUIAutomationValuePattern = element
                    .GetCurrentPatternAs(UIA_ValuePatternId)
                    .map_err(|e| pattern_failure("value_pattern", e))?;
                operation.begin_effect("set_value")?;
                pattern
                    .SetValue(&BSTR::from(value))
                    .map_err(|e| failure("set_value", e))?;
            }
            UiaAction::Focus => {
                operation.begin_effect("focus")?;
                element.SetFocus().map_err(|e| failure("focus", e))?;
            }
            UiaAction::Toggle => {
                let pattern: IUIAutomationTogglePattern = element
                    .GetCurrentPatternAs(UIA_TogglePatternId)
                    .map_err(|e| pattern_failure("toggle_pattern", e))?;
                operation.begin_effect("toggle")?;
                pattern.Toggle().map_err(|e| failure("toggle", e))?;
            }
            UiaAction::Selection(action) => {
                let pattern: IUIAutomationSelectionItemPattern = element
                    .GetCurrentPatternAs(UIA_SelectionItemPatternId)
                    .map_err(|e| pattern_failure("selection_pattern", e))?;
                operation.begin_effect("selection")?;
                match action {
                    SelectionAction::Select => pattern.Select(),
                    SelectionAction::Add => pattern.AddToSelection(),
                    SelectionAction::Remove => pattern.RemoveFromSelection(),
                }
                .map_err(|e| failure("selection", e))?;
            }
            UiaAction::Expand | UiaAction::Collapse => {
                let pattern: IUIAutomationExpandCollapsePattern = element
                    .GetCurrentPatternAs(UIA_ExpandCollapsePatternId)
                    .map_err(|e| pattern_failure("expand_pattern", e))?;
                operation.begin_effect("expand_collapse")?;
                if matches!(action, UiaAction::Expand) {
                    pattern.Expand()
                } else {
                    pattern.Collapse()
                }
                .map_err(|e| failure("expand_collapse", e))?;
            }
            UiaAction::Scroll { axis, amount } => {
                let pattern: IUIAutomationScrollPattern = element
                    .GetCurrentPatternAs(UIA_ScrollPatternId)
                    .map_err(|e| pattern_failure("scroll_pattern", e))?;
                let amount = match amount {
                    ScrollAmount::SmallDecrement => ScrollAmount_SmallDecrement,
                    ScrollAmount::SmallIncrement => ScrollAmount_SmallIncrement,
                    ScrollAmount::LargeDecrement => ScrollAmount_LargeDecrement,
                    ScrollAmount::LargeIncrement => ScrollAmount_LargeIncrement,
                };
                let (horizontal, vertical) = match axis {
                    ScrollAxis::Horizontal => (amount, ScrollAmount_NoAmount),
                    ScrollAxis::Vertical => (ScrollAmount_NoAmount, amount),
                };
                operation.begin_effect("scroll")?;
                pattern
                    .Scroll(horizontal, vertical)
                    .map_err(|e| failure("scroll", e))?;
            }
            UiaAction::ScrollIntoView => {
                let pattern: IUIAutomationScrollItemPattern = element
                    .GetCurrentPatternAs(UIA_ScrollItemPatternId)
                    .map_err(|e| pattern_failure("scroll_item_pattern", e))?;
                operation.begin_effect("scroll_into_view")?;
                pattern
                    .ScrollIntoView()
                    .map_err(|e| failure("scroll_into_view", e))?;
            }
        }
    }
    operation
        .check("uia_action_complete")
        .map_err(Failure::from)
}
