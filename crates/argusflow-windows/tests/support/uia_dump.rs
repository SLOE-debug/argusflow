//! E2E 失败诊断使用的有限深度 Control View dump 与 Value readback。

use std::ffi::c_void;

use argusflow_agent::WindowContext;
use windows::Win32::{
    Foundation::HWND,
    System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize, SAFEARRAY,
        },
        Ole::{
            SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound,
            SafeArrayGetUBound,
        },
        Variant::VARIANT,
    },
    UI::Accessibility::{
        CUIAutomation8, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker,
        TreeScope_Descendants, TreeScope_Subtree, UIA_IsExpandCollapsePatternAvailablePropertyId,
        UIA_IsInvokePatternAvailablePropertyId, UIA_IsLegacyIAccessiblePatternAvailablePropertyId,
        UIA_IsValuePatternAvailablePropertyId, UIA_PROPERTY_ID, UIA_ProcessIdPropertyId,
        UIA_ValueValuePropertyId,
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

/// 判断应用进程的全部 UIA fragment 是否存在指定 Value property，用于 SetValue readback。
pub(crate) fn has_value(window: &WindowContext, expected: &str) -> bool {
    try_has_value(window, expected).unwrap_or(false)
}

/// 判断指定进程的 UIA tree 是否存在指定中文名称前缀，用于弹出菜单状态断言。
pub(crate) fn has_name_prefix_for_process(process_id: u32, expected_prefix: &str) -> bool {
    try_has_name_prefix_for_process(process_id, expected_prefix).unwrap_or(false)
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
    // SAFETY: element 留在创建 apartment，仅同步读取 provider framework。
    let framework_id = unsafe { element.CurrentFrameworkId() }
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "<unavailable>".to_owned());
    // SAFETY: element 留在创建 apartment，仅同步读取命令快捷键。
    let accelerator_key = unsafe { element.CurrentAcceleratorKey() }
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "<unavailable>".to_owned());
    // SAFETY: element 留在创建 apartment，仅同步读取助记键。
    let access_key = unsafe { element.CurrentAccessKey() }
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "<unavailable>".to_owned());
    // SAFETY: element 留在创建 apartment，仅同步读取进程 ID 与屏幕矩形。
    let process_id = unsafe { element.CurrentProcessId() }.unwrap_or_default();
    // SAFETY: element 留在创建 apartment，返回结构立即复制为普通数值。
    let rectangle = unsafe { element.CurrentBoundingRectangle() }.ok();
    let runtime_id = read_runtime_id(element).unwrap_or_default();
    let patterns = [
        ("Invoke", UIA_IsInvokePatternAvailablePropertyId),
        (
            "ExpandCollapse",
            UIA_IsExpandCollapsePatternAvailablePropertyId,
        ),
        (
            "LegacyIAccessible",
            UIA_IsLegacyIAccessiblePatternAvailablePropertyId,
        ),
        ("Value", UIA_IsValuePatternAvailablePropertyId),
    ]
    .into_iter()
    .filter_map(|(name, property)| pattern_available(element, property).then_some(name))
    .collect::<Vec<_>>();
    // SAFETY: element 留在创建 apartment，仅同步读取当前 IsEnabled。
    let enabled = unsafe { element.CurrentIsEnabled() }
        .map(|value| value.as_bool())
        .unwrap_or(false);
    // SAFETY: element 留在创建 apartment，仅同步读取当前 IsOffscreen。
    let offscreen = unsafe { element.CurrentIsOffscreen() }
        .map(|value| value.as_bool())
        .unwrap_or(true);
    eprintln!(
        "{indent}pid={process_id} runtime_id={runtime_id:?} type={control_type} name={name:?} automation_id={automation_id:?} accelerator_key={accelerator_key:?} access_key={access_key:?} class={class_name:?} framework={framework_id:?} enabled={enabled} offscreen={offscreen} rectangle={rectangle:?} patterns={patterns:?}"
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

/// 读取 pattern availability；不可用或类型异常都只影响辅助 dump。
fn pattern_available(element: &IUIAutomationElement, property: UIA_PROPERTY_ID) -> bool {
    // SAFETY: property 来自固定 UIA pattern availability 集合。
    unsafe { element.GetCurrentPropertyValue(property) }
        .ok()
        .and_then(|value| bool::try_from(&value).ok())
        .unwrap_or(false)
}

/// 把 RuntimeId SAFEARRAY 拷贝成只在当前 dump 中使用的普通整数数组。
fn read_runtime_id(element: &IUIAutomationElement) -> windows::core::Result<Vec<i32>> {
    // SAFETY: element 留在当前测试 apartment，array 立即交给唯一 guard。
    let array = unsafe { element.GetRuntimeId() }?;
    if array.is_null() {
        return Ok(Vec::new());
    }
    let array = TestSafeArray(array);
    // SAFETY: array 来自 GetRuntimeId 且尚未释放。
    if unsafe { SafeArrayGetDim(array.0) } != 1 {
        return Ok(Vec::new());
    }
    // SAFETY: 已验证一维数组，维度索引 1 有效。
    let lower = unsafe { SafeArrayGetLBound(array.0, 1) }?;
    // SAFETY: 已验证一维数组，维度索引 1 有效。
    let upper = unsafe { SafeArrayGetUBound(array.0, 1) }?;
    let mut values = Vec::new();
    for index in lower..=upper {
        let mut value = 0_i32;
        // SAFETY: index 在已读取上下界内，输出指针有效且独占。
        unsafe { SafeArrayGetElement(array.0, &index, (&mut value as *mut i32).cast()) }?;
        values.push(value);
    }
    Ok(values)
}

/// 确保测试 RuntimeId SAFEARRAY 在当前 apartment 释放。
struct TestSafeArray(*mut SAFEARRAY);

impl Drop for TestSafeArray {
    fn drop(&mut self) {
        // SAFETY: 指针来自 GetRuntimeId 且只由本 guard 拥有。
        let _ = unsafe { SafeArrayDestroy(self.0) };
    }
}

/// 使用真实 ProcessId + Value PropertyCondition 在桌面树内读取应用值。
fn try_has_value(window: &WindowContext, expected: &str) -> windows::core::Result<bool> {
    let _apartment = TestApartment::initialize()?;
    // SAFETY: COM 已在当前测试线程初始化，client 不会离开本同步 helper。
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }?;
    // SAFETY: automation client 留在创建它的当前 apartment。
    let root = unsafe { automation.GetRootElement() }?;
    let value = VARIANT::from(expected);
    // SAFETY: Value property 与 BSTR VARIANT 类型匹配，value 在同步调用期间有效。
    let value_condition =
        unsafe { automation.CreatePropertyCondition(UIA_ValueValuePropertyId, &value) }?;
    // SAFETY: automation client 留在创建它的当前 apartment。
    let control_view = unsafe { automation.ControlViewCondition() }?;
    let process = VARIANT::from(window.process_id as i32);
    // SAFETY: ProcessId property 与 i32 VARIANT 类型匹配，process 在同步调用期间有效。
    let process_condition =
        unsafe { automation.CreatePropertyCondition(UIA_ProcessIdPropertyId, &process) }?;
    // SAFETY: 三个 condition 均由同一 automation client 创建。
    let value_in_control_view =
        unsafe { automation.CreateAndCondition(&control_view, &value_condition) }?;
    let condition =
        unsafe { automation.CreateAndCondition(&process_condition, &value_in_control_view) }?;
    // SAFETY: root/condition 同属当前 apartment，TreeScope_Subtree 是有效枚举值。
    let elements = unsafe { root.FindAll(TreeScope_Subtree, &condition) }?;
    // SAFETY: element array 未离开当前 apartment。
    unsafe { elements.Length() }.map(|length| length > 0)
}

/// 在 Desktop UIA tree 中按 ProcessId 收窄候选，并读取 CurrentName 检查中文前缀。
fn try_has_name_prefix_for_process(
    process_id: u32,
    expected_prefix: &str,
) -> windows::core::Result<bool> {
    let _apartment = TestApartment::initialize()?;
    // SAFETY: COM 已在当前测试线程初始化，client 不会离开本同步 helper。
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }?;
    // SAFETY: automation client 留在创建它的当前 apartment。
    let root = unsafe { automation.GetRootElement() }?;
    let process = VARIANT::from(process_id as i32);
    // SAFETY: ProcessId property 与 i32 VARIANT 类型匹配，process 在同步调用期间有效。
    let process_condition =
        unsafe { automation.CreatePropertyCondition(UIA_ProcessIdPropertyId, &process) }?;
    // SAFETY: root/condition 同属当前 apartment，桌面搜索由 ProcessId 原生条件严格限制。
    let elements = unsafe { root.FindAll(TreeScope_Descendants, &process_condition) }?;
    // SAFETY: element array 未离开当前 apartment。
    let length = unsafe { elements.Length() }?;
    for index in 0..length {
        // SAFETY: index 来自同一数组的有效长度，element 留在当前 apartment。
        let element = unsafe { elements.GetElement(index) }?;
        // SAFETY: CurrentName 在当前 apartment 同步读取并立即转换为 Rust String。
        if unsafe { element.CurrentName() }?
            .to_string()
            .starts_with(expected_prefix)
        {
            return Ok(true);
        }
    }
    Ok(false)
}
