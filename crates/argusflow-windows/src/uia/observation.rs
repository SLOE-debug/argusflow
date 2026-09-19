//! 定点／焦点只读观察，COM 始终留在既有 MTA。
use crate::{UiaRuntime, WindowsError, platform::failure};
use argusflow_core::{Operation, OperationOptions};
use serde::{Deserialize, Serialize};
use windows::Win32::{Foundation::POINT, UI::Accessibility::*};
/// 脱离 COM 的有限结构观察。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiaObservation {
    /// UIA RuntimeId，仅用于同一录制生命周期内的观察关联，不作为回放定位器。
    pub runtime_id: Vec<i32>,
    /// 目标的普通属性。
    pub target: UiaObservedNode,
    /// 至多四层祖先，包含相似控件的上下文。
    pub ancestors: Vec<UiaObservedNode>,
    /// 是否达到祖先／文字预算。
    pub truncated: bool,
    /// 目标控件的只读文本与选区；祖先不重复读取全文。
    pub text: super::text::UiaTextObservation,
}
/// 录制所需的相关属性，密码 Value 不读取。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiaObservedNode {
    /// 控件名。
    pub name: String,
    /// 自动化标识。
    pub automation_id: String,
    /// 类名。
    pub class_name: String,
    /// UIA 控件类型。
    pub role: i32,
    /// 元素进程。
    pub pid: i32,
    /// 屏幕物理边界。
    pub bounds: [i32; 4],
    /// 是否启用。
    pub enabled: bool,
    /// 当前焦点。
    pub focused: bool,
    /// 密码字段。
    pub password: bool,
    /// 实际 ValuePattern 值，不是键码拼接。
    pub value: Option<String>,
}
impl UiaRuntime {
    /// point=None 表示读取当前焦点；不切换焦点、不注入输入。
    pub async fn observe_target(
        &self,
        point: Option<[i32; 2]>,
        options: OperationOptions,
    ) -> Result<UiaObservation, WindowsError> {
        match self
            .call(super::worker::Command::Observe(point), options)
            .await?
        {
            super::worker::Response::Observation(value) => Ok(value),
            _ => Err(WindowsError::new(
                argusflow_core::FailureKind::Protocol,
                "uia_observe",
                "响应类型错误",
            )),
        }
    }
}
pub(super) fn observe(
    automation: &IUIAutomation2,
    point: Option<[i32; 2]>,
    operation: &Operation,
) -> Result<UiaObservation, WindowsError> {
    // SAFETY: 此函数仅由拥有 automation 的 MTA worker 调用。
    let target = unsafe {
        match point {
            Some([x, y]) => automation.ElementFromPoint(POINT { x, y }),
            None => automation.GetFocusedElement(),
        }
    }
    .map_err(|e| failure("uia_target", e))?;
    let mut truncated = false;
    let node = read(&target, &mut truncated)?;
    let runtime_id = super::text::identity(&target)?;
    let text = super::text::observe(&target, node.password, operation);
    let walker =
        unsafe { automation.ControlViewWalker() }.map_err(|e| failure("uia_ancestors", e))?;
    let mut ancestors = vec![];
    let mut current = target;
    for _ in 0..4 {
        operation.check("uia_ancestors")?;
        match unsafe { walker.GetParentElement(&current) } {
            Ok(parent) => {
                ancestors.push(read(&parent, &mut truncated)?);
                current = parent;
            }
            Err(error) if error.code().is_ok() => {
                return Ok(UiaObservation {
                    runtime_id,
                    target: node,
                    ancestors,
                    truncated,
                    text,
                });
            }
            Err(error) => return Err(failure("uia_parent", error)),
        }
    }
    truncated = true;
    operation.check("uia_observe_complete")?;
    Ok(UiaObservation {
        runtime_id,
        target: node,
        ancestors,
        truncated,
        text,
    })
}
fn read(
    element: &IUIAutomationElement,
    truncated: &mut bool,
) -> Result<UiaObservedNode, WindowsError> {
    // SAFETY: 所有属性和 Pattern 访问均在原 MTA 中，无原生对象逃逸。
    unsafe {
        let password = element
            .CurrentIsPassword()
            .map_err(|e| failure("uia_password", e))?
            .as_bool();
        let snapshot = super::query::snapshot(element)?;
        let value = if password {
            None
        } else {
            match element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) {
                Ok(pattern) => Some(bounded(
                    pattern
                        .CurrentValue()
                        .map_err(|e| failure("uia_value", e))?
                        .to_string(),
                    truncated,
                )),
                Err(error)
                    if error.code() == windows::Win32::Foundation::E_NOINTERFACE
                        || error.code().is_ok()
                        || error.code().0 as u32 == UIA_E_NOTSUPPORTED =>
                {
                    None
                }
                Err(error) => return Err(failure("uia_value", error)),
            }
        };
        Ok(UiaObservedNode {
            name: if password {
                "[密码字段]".into()
            } else {
                bounded(snapshot.name, truncated)
            },
            automation_id: bounded(snapshot.automation_id, truncated),
            class_name: bounded(snapshot.class_name, truncated),
            role: snapshot.control_type,
            pid: element
                .CurrentProcessId()
                .map_err(|e| failure("uia_pid", e))?,
            bounds: snapshot.bounds,
            enabled: snapshot.enabled,
            focused: element
                .CurrentHasKeyboardFocus()
                .map_err(|e| failure("uia_focus", e))?
                .as_bool(),
            password,
            value,
        })
    }
}
fn bounded(value: String, truncated: &mut bool) -> String {
    if value.len() > 8192 {
        *truncated = true;
        value.chars().take(2048).collect()
    } else {
        value
    }
}
