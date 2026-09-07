//! 在现有 MTA worker 中执行 reverse hit-test，绝不读取 ValuePattern。

use argusflow_core::{
    ElementSemantics, FieldSensitivity, InspectedEntity, InspectionContext, InspectionFailure,
    InspectionProbe, InspectionRect, TargetInspector, sensitive_field_metadata,
};
use async_trait::async_trait;
use tokio::sync::oneshot;
use windows::Win32::{
    Foundation::{HWND, POINT},
    UI::{
        Accessibility::*,
        HiDpi::{
            DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            SetThreadDpiAwarenessContext,
        },
        WindowsAndMessaging::{GA_ROOT, GetAncestor, GetWindowThreadProcessId},
    },
};

use super::{element_identity::runtime_id, runtime::UiaRuntime};

#[async_trait]
impl TargetInspector for UiaRuntime {
    async fn inspect(
        &self,
        context: &InspectionContext,
        probe: InspectionProbe,
    ) -> Result<InspectedEntity, InspectionFailure> {
        if !self.health.is_ready() {
            return Err(InspectionFailure::Unavailable);
        }
        let (sender, receiver) = oneshot::channel();
        let generation = {
            let worker = self
                .worker
                .lock()
                .map_err(|_| InspectionFailure::Unavailable)?;
            worker.send_inspect(context.clone(), probe, sender)?;
            worker.generation()
        };
        // 录制请求使用短预算。第三方 provider 阻塞时沿用 runtime 的有限 generation 恢复。
        match tokio::time::timeout(std::time::Duration::from_millis(800), receiver).await {
            Ok(Ok(result)) => result,
            _ => {
                self.recover_worker(generation);
                Err(InspectionFailure::Timeout)
            }
        }
    }
}

/// 只在已初始化的 UIA apartment 内使用 cache 和元素；返回值不含 COM 接口。
pub(super) fn inspect_element(
    automation: &IUIAutomation,
    context: &InspectionContext,
    probe: InspectionProbe,
) -> Result<InspectedEntity, InspectionFailure> {
    // UIA 的 Win32 proxy 会受调用线程 DPI 虚拟化影响；输入与 cache bounds 必须同为物理像素。
    let _dpi = InspectionDpiScope::enter();
    let hwnd = HWND(context.window.handle as *mut _);
    let mut process_id = 0;
    // SAFETY: 只读复验冻结 HWND/PID；所有 COM 对象保持在创建它们的 worker。
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    if process_id != context.window.process_id {
        return Err(InspectionFailure::ContextChanged);
    }
    let request = unsafe { automation.CreateCacheRequest() }.map_err(unavailable)?;
    for property in [
        UIA_ControlTypePropertyId,
        UIA_NamePropertyId,
        UIA_AutomationIdPropertyId,
        UIA_ClassNamePropertyId,
        UIA_FrameworkIdPropertyId,
        UIA_BoundingRectanglePropertyId,
        UIA_IsPasswordPropertyId,
        UIA_ProcessIdPropertyId,
        UIA_NativeWindowHandlePropertyId,
        UIA_IsOffscreenPropertyId,
        UIA_HasKeyboardFocusPropertyId,
    ] {
        unsafe { request.AddProperty(property) }.map_err(unavailable)?;
    }
    unsafe { request.SetTreeScope(TreeScope_Element) }.map_err(unavailable)?;
    let element = unsafe {
        match probe {
            InspectionProbe::Point(point) => automation.ElementFromPointBuildCache(
                POINT {
                    x: point.x,
                    y: point.y,
                },
                &request,
            ),
            InspectionProbe::Focus => automation.GetFocusedElementBuildCache(&request),
        }
    }
    .map_err(|_| InspectionFailure::NoElement)?;
    let rect = unsafe { element.CachedBoundingRectangle() }.map_err(unavailable)?;
    let bounds = InspectionRect {
        x: f64::from(rect.left),
        y: f64::from(rect.top),
        width: f64::from(rect.right - rect.left),
        height: f64::from(rect.bottom - rect.top),
    };
    if !bounds.is_valid()
        || unsafe { element.CachedIsOffscreen() }
            .map_err(unavailable)?
            .as_bool()
        || matches!(probe, InspectionProbe::Point(point) if !bounds.contains(point))
    {
        return Err(InspectionFailure::NoElement);
    }
    let mut semantics = read_semantics(&element)?;
    let password = unsafe { element.CachedIsPassword() }
        .map_err(unavailable)?
        .as_bool();
    let sensitive = password
        || [&semantics.name, &semantics.automation_id]
            .into_iter()
            .flatten()
            .any(|value| sensitive_field_metadata(value));
    let walker = unsafe { automation.ControlViewWalker() }.map_err(unavailable)?;
    let mut current = element.clone();
    let mut ancestors = Vec::new();
    let mut belongs_to_window = false;
    for depth in 0..16 {
        let native = unsafe { current.CachedNativeWindowHandle() }.map_err(unavailable)?;
        if !native.0.is_null() && unsafe { GetAncestor(native, GA_ROOT) } == hwnd {
            belongs_to_window = true;
        }
        if depth > 0 {
            let mut ancestor = read_semantics(&current)?;
            // 容器 Accessible Name 可能包含子输入的聚合文本，只使用其标识构造祖先关系。
            ancestor.name = None;
            ancestors.push(ancestor);
        }
        if native == hwnd {
            break;
        }
        let Ok(parent) = (unsafe { walker.GetParentElementBuildCache(&current, &request) }) else {
            break;
        };
        current = parent;
    }
    if !belongs_to_window {
        return Err(InspectionFailure::ContextChanged);
    }
    if semantics.role.is_none()
        || (semantics.name.is_none()
            && semantics.automation_id.is_none()
            && semantics.role != Some(argusflow_core::ElementRole::TextBox))
    {
        return Err(InspectionFailure::NoElement);
    }
    if sensitive {
        semantics.name = None;
        ancestors.iter_mut().for_each(|item| item.name = None);
    }
    Ok(InspectedEntity {
        identity: format!(
            "uia:{:?}",
            runtime_id(&element).map_err(|_| InspectionFailure::NoElement)?
        ),
        editable: semantics.role == Some(argusflow_core::ElementRole::TextBox),
        semantics,
        ancestors,
        bounds,
        sensitivity: if sensitive {
            FieldSensitivity::Sensitive
        } else {
            FieldSensitivity::Normal
        },
        browser_session: None,
        page_url: None,
        confidence: 0.9,
    })
}

/// 所有字段已加入同一个 CacheRequest；不读取潜在敏感的 Value。
fn read_semantics(element: &IUIAutomationElement) -> Result<ElementSemantics, InspectionFailure> {
    // SAFETY: cache 在同一个 MTA worker 创建，读取的属性均明确注册。
    unsafe {
        Ok(ElementSemantics {
            role: super::inspection_role::role(element.CachedControlType().map_err(unavailable)?),
            name: non_empty(element.CachedName().map_err(unavailable)?.to_string()),
            automation_id: non_empty(
                element
                    .CachedAutomationId()
                    .map_err(unavailable)?
                    .to_string(),
            ),
            class_name: non_empty(element.CachedClassName().map_err(unavailable)?.to_string()),
            framework_id: non_empty(
                element
                    .CachedFrameworkId()
                    .map_err(unavailable)?
                    .to_string(),
            ),
            ..ElementSemantics::default()
        })
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
fn unavailable(_: windows::core::Error) -> InspectionFailure {
    InspectionFailure::Unavailable
}

/// 只覆盖同步 UIA 反查；不改变复用 worker 后续执行操作的 DPI 约定。
struct InspectionDpiScope(DPI_AWARENESS_CONTEXT);

impl InspectionDpiScope {
    fn enter() -> Self {
        // SAFETY: 本 guard 不跨 await，Drop 在同一个 MTA 线程恢复原上下文。
        Self(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) })
    }
}

impl Drop for InspectionDpiScope {
    fn drop(&mut self) {
        if !self.0.0.is_null() {
            // SAFETY: 值来自本线程的 SetThreadDpiAwarenessContext。
            unsafe {
                SetThreadDpiAwarenessContext(self.0);
            }
        }
    }
}
