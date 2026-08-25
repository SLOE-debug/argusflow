//! E2E 失败诊断使用的有限深度 Control View dump 与 Value readback。

use std::ffi::c_void;

use argusflow_agent::WindowContext;
use windows::Win32::{
    Foundation::HWND,
    System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        Variant::VARIANT,
    },
    UI::Accessibility::{
        CUIAutomation8, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker,
        TreeScope_Subtree, UIA_ValueValuePropertyId,
    },
};

/// 只在测试 helper 当前线程存活的 COM apartment。
struct TestApartment;

impl TestApartment {
    /// 初始化 MTA；失败时由调用方放弃辅助诊断。
    fn initialize() -> windows::core::Result<Self> {
        // SAFETY: helper 在当前测试线程同步使用，并由 TestApartment::drop 配对清理。
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }?;
        Ok(Self)
    }
}

impl Drop for TestApartment {
    fn drop(&mut self) {
        // SAFETY: guard 与成功的 CoInitializeEx 位于同一测试线程。
        unsafe { CoUninitialize() };
    }
}

/// 仅在测试失败时输出有限深度 UIA Control View。
pub(crate) fn dump_control_view(window: &WindowContext, max_depth: usize) {
    if let Err(error) = try_dump_control_view(window, max_depth) {
        eprintln!("UIA tree dump failed: {error}");
    }
}

/// 判断窗口子树是否存在指定 Value property，用于 SetValue readback。
pub(crate) fn has_value(window: &WindowContext, expected: &str) -> bool {
    try_has_value(window, expected).unwrap_or(false)
}

/// 创建临时测试 client 并递归输出节点摘要。
fn try_dump_control_view(window: &WindowContext, max_depth: usize) -> windows::core::Result<()> {
    let _apartment = TestApartment::initialize()?;
    // SAFETY: COM 已在当前测试线程初始化，client 不会离开本同步 helper。
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }?;
    // SAFETY: HWND 来自 fixture 的 PID scoped 顶层窗口，仅作为 UIA root 输入。
    let root =
        unsafe { automation.ElementFromHandle(HWND(window.handle as usize as *mut c_void)) }?;
    // SAFETY: automation client 留在创建它的当前 apartment。
    let walker = unsafe { automation.ControlViewWalker() }?;
    // 限制异常 provider 在同一层返回过多节点时的诊断体积。
    let mut remaining_nodes = 500_usize;
    eprintln!("UIA Control View for hwnd={}", window.handle);
    dump_element(&walker, &root, 0, max_depth, &mut remaining_nodes);
    Ok(())
}

/// 输出当前节点，并使用 ControlViewWalker 遍历有限深度子树。
fn dump_element(
    walker: &IUIAutomationTreeWalker,
    element: &IUIAutomationElement,
    depth: usize,
    max_depth: usize,
    remaining_nodes: &mut usize,
) {
    if *remaining_nodes == 0 {
        return;
    }
    *remaining_nodes -= 1;
    let indent = "  ".repeat(depth);
    // SAFETY: 以下只读 getter 都在 element 的创建 apartment 内同步调用。
    let control_type = unsafe { element.CurrentControlType() }
        .map(|value| value.0)
        .unwrap_or_default();
    // SAFETY: element 留在创建 apartment，仅同步读取当前 Name。
    let name = unsafe { element.CurrentName() }
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "<unavailable>".to_owned());
    // SAFETY: element 留在创建 apartment，仅同步读取当前 AutomationId。
    let automation_id = unsafe { element.CurrentAutomationId() }
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "<unavailable>".to_owned());
    // SAFETY: element 留在创建 apartment，仅同步读取当前 ClassName。
    let class_name = unsafe { element.CurrentClassName() }
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "<unavailable>".to_owned());
    // SAFETY: element 留在创建 apartment，仅同步读取当前 IsEnabled。
    let enabled = unsafe { element.CurrentIsEnabled() }
        .map(|value| value.as_bool())
        .unwrap_or(false);
    // SAFETY: element 留在创建 apartment，仅同步读取当前 IsOffscreen。
    let offscreen = unsafe { element.CurrentIsOffscreen() }
        .map(|value| value.as_bool())
        .unwrap_or(true);
    eprintln!(
        "{indent}type={control_type} name={name:?} automation_id={automation_id:?} class={class_name:?} enabled={enabled} offscreen={offscreen}"
    );
    if depth >= max_depth {
        return;
    }
    // SAFETY: walker 与 element 来自同一个 automation client/apartment。
    let Ok(mut child) = (unsafe { walker.GetFirstChildElement(element) }) else {
        return;
    };
    loop {
        dump_element(walker, &child, depth + 1, max_depth, remaining_nodes);
        if *remaining_nodes == 0 {
            break;
        }
        // SAFETY: walker 与当前 child 来自同一个 automation client/apartment。
        match unsafe { walker.GetNextSiblingElement(&child) } {
            Ok(sibling) => child = sibling,
            Err(_) => break,
        }
    }
}

/// 使用真实 PropertyCondition 在 HWND 子树内读取 Value property。
fn try_has_value(window: &WindowContext, expected: &str) -> windows::core::Result<bool> {
    let _apartment = TestApartment::initialize()?;
    // SAFETY: COM 已在当前测试线程初始化，client 不会离开本同步 helper。
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }?;
    // SAFETY: HWND 来自 fixture 的 PID scoped 顶层窗口，仅作为 UIA root 输入。
    let root =
        unsafe { automation.ElementFromHandle(HWND(window.handle as usize as *mut c_void)) }?;
    let value = VARIANT::from(expected);
    // SAFETY: Value property 与 BSTR VARIANT 类型匹配，value 在同步调用期间有效。
    let value_condition =
        unsafe { automation.CreatePropertyCondition(UIA_ValueValuePropertyId, &value) }?;
    // SAFETY: automation client 留在创建它的当前 apartment。
    let control_view = unsafe { automation.ControlViewCondition() }?;
    // SAFETY: 两个 condition 均由同一 automation client 创建。
    let condition = unsafe { automation.CreateAndCondition(&control_view, &value_condition) }?;
    // SAFETY: root/condition 同属当前 apartment，TreeScope_Subtree 是有效枚举值。
    let elements = unsafe { root.FindAll(TreeScope_Subtree, &condition) }?;
    // SAFETY: element array 未离开当前 apartment。
    unsafe { elements.Length() }.map(|length| length > 0)
}
