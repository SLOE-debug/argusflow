//! 点击前重新获取位置并验证 UIA 命中归属。
use super::super::query::{optional_navigation, snapshot};
use crate::{WindowsError, platform::failure};
use argusflow_core::{FailureKind, Operation, ScreenPoint};
use windows::Win32::{
    Foundation::POINT,
    UI::Accessibility::{IUIAutomation2, IUIAutomationElement},
};

pub(crate) fn click_point(
    automation: &IUIAutomation2,
    element: &IUIAutomationElement,
    operation: &Operation,
) -> Result<ScreenPoint, WindowsError> {
    operation.check("uia_click_point")?;
    let value = snapshot(element)?;
    let [left, top, right, bottom] = value.bounds;
    if !value.enabled || value.offscreen || left >= right || top >= bottom {
        return Err(invalid("目标不可用、不可见或没有有效位置"));
    }
    let point = ScreenPoint {
        x: ((i64::from(left) + i64::from(right)) / 2) as i32,
        y: ((i64::from(top) + i64::from(bottom)) / 2) as i32,
    };
    // SAFETY: 命中测试和父级导航使用当前 MTA，与原元素身份比较。
    unsafe {
        let mut hit = Some(
            automation
                .ElementFromPoint(POINT {
                    x: point.x,
                    y: point.y,
                })
                .map_err(|e| failure("uia_hit_test", e))?,
        );
        let walker = automation
            .RawViewWalker()
            .map_err(|e| failure("uia_hit_walker", e))?;
        for _ in 0..64 {
            operation.check("uia_hit_test")?;
            let Some(current) = hit else {
                break;
            };
            if automation
                .CompareElements(element, &current)
                .map_err(|e| failure("uia_hit_compare", e))?
                .as_bool()
            {
                return Ok(point);
            }
            hit = optional_navigation(walker.GetParentElement(&current))?;
        }
    }
    Err(invalid("目标中心被其他 UIA 元素遮挡"))
}
fn invalid(message: &str) -> WindowsError {
    WindowsError::new(FailureKind::InvalidInput, "uia_click_point", message)
}
