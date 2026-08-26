//! UIA process-scoped Failure Evidence 快照采集。

use std::{collections::HashMap, path::PathBuf, time::Instant};

use argusflow_agent::{
    EvidenceArtifact, EvidenceArtifactData, EvidenceArtifactKind, EvidenceBundle,
    EvidenceCaptureError, EvidenceCaptureRequest,
};
use argusflow_core::BackendKind;
use serde::Serialize;
use serde_json::json;
use windows::{
    Win32::{
        System::Variant::VARIANT,
        UI::Accessibility::{
            IUIAutomation, IUIAutomationElement, TreeScope_Descendants, UIA_IsDialogPropertyId,
            UIA_IsExpandCollapsePatternAvailablePropertyId, UIA_IsInvokePatternAvailablePropertyId,
            UIA_IsLegacyIAccessiblePatternAvailablePropertyId,
            UIA_IsValuePatternAvailablePropertyId, UIA_ProcessIdPropertyId,
            UIA_ToggleToggleStatePropertyId, UIA_ValueValuePropertyId,
        },
    },
    core::BSTR,
};

use super::{
    element_identity::runtime_id,
    error::{UiaError, UiaOperation},
    plan::UiaPreparedPlan,
    runtime::PreparedWindowTarget,
    selector_trace::{SelectorTrace, build_selector_trace},
};

/// 普通 Rust DTO 表示的 UIA bounding rectangle。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(super) struct UiaRectangleSnapshot {
    /// 屏幕坐标左边界。
    pub(super) left: i32,
    /// 屏幕坐标上边界。
    pub(super) top: i32,
    /// 矩形宽度。
    pub(super) width: i32,
    /// 矩形高度。
    pub(super) height: i32,
}

/// Evidence 中记录的有限动作 pattern 集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum UiaPatternSnapshot {
    /// InvokePattern。
    Invoke,
    /// ExpandCollapsePattern。
    ExpandCollapse,
    /// LegacyIAccessiblePattern。
    LegacyIAccessible,
    /// ValuePattern。
    Value,
}

/// 单个 UIA Control View 节点的脱离 COM 的诊断快照。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct UiaNodeSnapshot {
    /// 只在当前 snapshot 内关联节点的 runtime id。
    pub(super) runtime_id: Vec<i32>,
    /// Control View parent 的 runtime id；进程根 fragment 为 None。
    pub(super) parent_runtime_id: Option<Vec<i32>>,
    /// 从当前进程 fragment 根开始的近似深度。
    pub(super) depth: usize,
    /// provider 报告的进程 ID。
    pub(super) process_id: i32,
    /// 原生 ControlType id。
    pub(super) control_type: i32,
    /// provider 本地化控件类型。
    pub(super) localized_control_type: String,
    /// Accessible Name。
    pub(super) name: String,
    /// AutomationId。
    pub(super) automation_id: String,
    /// 原生窗口类名。
    pub(super) class_name: String,
    /// provider framework id。
    pub(super) framework_id: String,
    /// 命令快捷键。
    pub(super) accelerator_key: String,
    /// 菜单或控件助记键。
    pub(super) access_key: String,
    /// 仅在 retention policy 允许且控件不是密码字段时保存。
    pub(super) value: Option<String>,
    /// 是否为密码或受保护控件。
    pub(super) is_password: bool,
    /// 是否可交互。
    pub(super) is_enabled: bool,
    /// 是否位于屏幕外。
    pub(super) is_offscreen: bool,
    /// 是否拥有键盘焦点。
    pub(super) has_keyboard_focus: bool,
    /// ToggleState 原生枚举值。
    pub(super) toggle_state: i32,
    /// 是否为 SelectionItem 当前选中项。
    pub(super) is_selected: bool,
    /// Window 是否表达 Dialog 语义。
    pub(super) is_dialog: bool,
    /// 屏幕坐标矩形。
    pub(super) bounding_rectangle: UiaRectangleSnapshot,
    /// 与动作执行有关的可用 patterns。
    pub(super) available_patterns: Vec<UiaPatternSnapshot>,
}

/// 只在 UIA worker apartment 内同步使用的证据采集器。
pub(super) struct UiaEvidenceCollector<'automation> {
    /// worker 创建并拥有的 automation client。
    automation: &'automation IUIAutomation,
}

impl<'automation> UiaEvidenceCollector<'automation> {
    /// 绑定 worker automation client。
    pub(super) const fn new(automation: &'automation IUIAutomation) -> Self {
        Self { automation }
    }

    /// 按真实 executor 的 Desktop root + ProcessId 边界采集证据。
    pub(super) fn capture(
        &self,
        window: PreparedWindowTarget,
        plan: &UiaPreparedPlan,
        query: &str,
        request: EvidenceCaptureRequest,
    ) -> Result<EvidenceBundle, EvidenceCaptureError> {
        if request.explain.backend != BackendKind::WindowsUia {
            return Err(EvidenceCaptureError::BackendMismatch);
        }
        let started_at = Instant::now();
        let nodes = self
            .capture_process_nodes(window.process_id, &request, started_at)
            .map_err(capture_error)?;
        let trace = build_selector_trace(
            request.explain.branch_path.clone().unwrap_or_default(),
            window.process_id,
            &plan.query.expression,
            &nodes,
            request.budget.max_near_misses,
        );
        self.build_bundle(window, query, request, nodes, trace)
    }

    /// 通过 ControlViewCondition + ProcessId 在桌面根发现全部独立 provider fragments。
    fn capture_process_nodes(
        &self,
        process_id: u32,
        request: &EvidenceCaptureRequest,
        started_at: Instant,
    ) -> Result<Vec<UiaNodeSnapshot>, UiaError> {
        let process_id =
            i32::try_from(process_id).map_err(|_| UiaError::InvalidProcessId { process_id })?;
        // SAFETY: automation 与 desktop root 都留在当前 UIA worker apartment。
        let desktop = unsafe { self.automation.GetRootElement() }
            .map_err(|source| UiaError::from_native(UiaOperation::GetDesktopRoot, source))?;
        let process_value = VARIANT::from(process_id);
        // SAFETY: ProcessId property 接受 VT_I4，VARIANT 在同步调用期间有效。
        let process_condition = unsafe {
            self.automation
                .CreatePropertyCondition(UIA_ProcessIdPropertyId, &process_value)
        }
        .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))?;
        // SAFETY: condition 由当前 automation client 创建。
        let control_view = unsafe { self.automation.ControlViewCondition() }
            .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))?;
        // SAFETY: 两个 condition 均来自同一 automation client。
        let condition = unsafe {
            self.automation
                .CreateAndCondition(&process_condition, &control_view)
        }
        .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))?;
        // SAFETY: Desktop root 与 condition 同属当前 apartment。
        let elements = unsafe { desktop.FindAll(TreeScope_Descendants, &condition) }
            .map_err(|source| UiaError::from_native(UiaOperation::FindAll, source))?;
        // SAFETY: element array 没有离开当前 apartment。
        let length = unsafe { elements.Length() }
            .map_err(|source| UiaError::from_native(UiaOperation::FindAll, source))?;
        let length = usize::try_from(length)
            .map_err(|_| UiaError::InvalidCandidateCount { count: length })?
            .min(request.budget.max_nodes);
        // SAFETY: walker 与全部候选属于当前 automation client。
        let walker = unsafe { self.automation.ControlViewWalker() }
            .map_err(|source| UiaError::from_native(UiaOperation::NavigateTree, source))?;
        let mut depths = HashMap::<Vec<i32>, usize>::new();
        let mut nodes = Vec::with_capacity(length);
        for index in 0..length {
            if started_at.elapsed() >= request.budget.deadline {
                return Err(UiaError::ExecutionDeadlineExceeded);
            }
            // SAFETY: index 来自已验证并收窄的 element array 长度。
            let element = unsafe { elements.GetElement(index as i32) }
                .map_err(|source| UiaError::from_native(UiaOperation::FindAll, source))?;
            let Ok(node) = snapshot_element(
                &walker,
                &element,
                process_id,
                &depths,
                request.retention.persist_text_values,
            ) else {
                // 单个异常 provider 节点不能擦掉同一进程其余可用现场。
                continue;
            };
            depths.insert(node.runtime_id.clone(), node.depth);
            if node.depth <= request.budget.max_depth {
                nodes.push(node);
            }
        }
        Ok(nodes)
    }

    /// 组合 planner、执行上下文、process tree 与 selector trace artifacts。
    fn build_bundle(
        &self,
        window: PreparedWindowTarget,
        query: &str,
        request: EvidenceCaptureRequest,
        nodes: Vec<UiaNodeSnapshot>,
        trace: SelectorTrace,
    ) -> Result<EvidenceBundle, EvidenceCaptureError> {
        let branch_path = request.explain.branch_path.clone().unwrap_or_default();
        let planner = serde_json::to_value(&request.explain).map_err(serialize_error)?;
        let tree = serde_json::to_value(&nodes).map_err(serialize_error)?;
        let candidates = serde_json::to_value(&trace.candidates).map_err(serialize_error)?;
        let trace = serde_json::to_value(&trace).map_err(serialize_error)?;
        let mut bundle =
            EvidenceBundle::new(BackendKind::WindowsUia, branch_path, request.trigger, query);
        bundle.push(json_artifact(
            EvidenceArtifactKind::PlannerExplain,
            "planner.json",
            false,
            planner,
        ));
        bundle.push(json_artifact(
            EvidenceArtifactKind::ExecutionContext,
            "context.json",
            false,
            json!({ "handle": window.handle, "process_id": window.process_id }),
        ));
        bundle.push(json_artifact(
            EvidenceArtifactKind::SelectorTrace,
            "selector_trace.json",
            false,
            trace,
        ));
        bundle.push(json_artifact(
            EvidenceArtifactKind::UiaProcessTree,
            "backend/uia/process_tree.json",
            true,
            tree,
        ));
        bundle.push(json_artifact(
            EvidenceArtifactKind::UiaCandidateSet,
            "backend/uia/candidates.json",
            true,
            candidates,
        ));
        Ok(bundle)
    }
}

/// 读取单元素完整诊断字段，并建立 snapshot 内 parent/depth 关联。
fn snapshot_element(
    walker: &windows::Win32::UI::Accessibility::IUIAutomationTreeWalker,
    element: &IUIAutomationElement,
    expected_process_id: i32,
    depths: &HashMap<Vec<i32>, usize>,
    persist_text_values: bool,
) -> Result<UiaNodeSnapshot, UiaError> {
    let element_runtime_id = runtime_id(element)?;
    // SAFETY: walker 与 element 同属 worker apartment；root parent 的空结果按 None 处理。
    let parent = optional_element(unsafe { walker.GetParentElement(element) })?;
    let parent_runtime_id = parent
        .as_ref()
        .filter(|parent| {
            current_i32(parent, UIA_ProcessIdPropertyId).ok() == Some(expected_process_id)
        })
        .and_then(|parent| runtime_id(parent).ok());
    let depth = parent_runtime_id
        .as_ref()
        .and_then(|parent| depths.get(parent))
        .map_or(0, |depth| depth.saturating_add(1));
    // SAFETY: element 留在 worker apartment，仅同步读取标准 UIA properties。
    let rectangle = unsafe { element.CurrentBoundingRectangle() }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?;
    let is_password =
        current_bool_method(element, |element| unsafe { element.CurrentIsPassword() })?;
    let value = if persist_text_values && !is_password {
        current_string(element, UIA_ValueValuePropertyId).ok()
    } else {
        None
    };
    Ok(UiaNodeSnapshot {
        runtime_id: element_runtime_id,
        parent_runtime_id,
        depth,
        process_id: current_i32(element, UIA_ProcessIdPropertyId)?,
        // SAFETY: element 留在 worker apartment。
        control_type: unsafe { element.CurrentControlType() }
            .map(|value| value.0)
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?,
        // SAFETY: string getters 在同步调用内立即拥有化。
        localized_control_type: unsafe { element.CurrentLocalizedControlType() }
            .map(|value| value.to_string())
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?,
        name: unsafe { element.CurrentName() }
            .map(|value| value.to_string())
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?,
        automation_id: unsafe { element.CurrentAutomationId() }
            .map(|value| value.to_string())
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?,
        class_name: unsafe { element.CurrentClassName() }
            .map(|value| value.to_string())
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?,
        framework_id: unsafe { element.CurrentFrameworkId() }
            .map(|value| value.to_string())
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?,
        accelerator_key: unsafe { element.CurrentAcceleratorKey() }
            .map(|value| value.to_string())
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?,
        access_key: unsafe { element.CurrentAccessKey() }
            .map(|value| value.to_string())
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?,
        value,
        is_password,
        is_enabled: current_bool_method(element, |element| unsafe { element.CurrentIsEnabled() })?,
        is_offscreen: current_bool_method(element, |element| unsafe {
            element.CurrentIsOffscreen()
        })?,
        has_keyboard_focus: current_bool_method(element, |element| unsafe {
            element.CurrentHasKeyboardFocus()
        })?,
        toggle_state: current_i32(element, UIA_ToggleToggleStatePropertyId).unwrap_or_default(),
        is_selected: current_bool_method(element, |element| unsafe {
            element.CurrentIsControlElement()
        })? && current_bool(
            element,
            windows::Win32::UI::Accessibility::UIA_SelectionItemIsSelectedPropertyId,
        )
        .unwrap_or(false),
        is_dialog: current_bool(element, UIA_IsDialogPropertyId).unwrap_or(false),
        bounding_rectangle: UiaRectangleSnapshot {
            left: rectangle.left,
            top: rectangle.top,
            width: rectangle.right.saturating_sub(rectangle.left),
            height: rectangle.bottom.saturating_sub(rectangle.top),
        },
        available_patterns: available_patterns(element)?,
    })
}

/// 读取与动作策略有关的 pattern availability。
fn available_patterns(element: &IUIAutomationElement) -> Result<Vec<UiaPatternSnapshot>, UiaError> {
    let candidates = [
        (
            UIA_IsInvokePatternAvailablePropertyId,
            UiaPatternSnapshot::Invoke,
        ),
        (
            UIA_IsExpandCollapsePatternAvailablePropertyId,
            UiaPatternSnapshot::ExpandCollapse,
        ),
        (
            UIA_IsLegacyIAccessiblePatternAvailablePropertyId,
            UiaPatternSnapshot::LegacyIAccessible,
        ),
        (
            UIA_IsValuePatternAvailablePropertyId,
            UiaPatternSnapshot::Value,
        ),
    ];
    let mut patterns = Vec::new();
    for (availability_property, pattern) in candidates {
        if current_bool(element, availability_property)? {
            patterns.push(pattern);
        }
    }
    Ok(patterns)
}

/// 读取标准 bool property。
fn current_bool(
    element: &IUIAutomationElement,
    property: windows::Win32::UI::Accessibility::UIA_PROPERTY_ID,
) -> Result<bool, UiaError> {
    // SAFETY: property 是封闭的标准 bool 属性，element 留在 worker apartment。
    let value = unsafe { element.GetCurrentPropertyValue(property) }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?;
    bool::try_from(&value).map_err(|_| UiaError::PatternAvailabilityTypeMismatch { property })
}

/// 统一拥有化 typed BOOL getter 的返回值。
fn current_bool_method(
    element: &IUIAutomationElement,
    getter: impl FnOnce(&IUIAutomationElement) -> windows::core::Result<windows::core::BOOL>,
) -> Result<bool, UiaError> {
    getter(element)
        .map(|value| value.as_bool())
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))
}

/// 读取标准 i32 property。
fn current_i32(
    element: &IUIAutomationElement,
    property: windows::Win32::UI::Accessibility::UIA_PROPERTY_ID,
) -> Result<i32, UiaError> {
    // SAFETY: property 是封闭的整数属性，element 留在 worker apartment。
    let value = unsafe { element.GetCurrentPropertyValue(property) }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?;
    i32::try_from(&value).map_err(|_| UiaError::PropertyTypeMismatch {
        property: super::native::UiaProperty::ToggleState,
    })
}

/// 读取标准 BSTR property。
fn current_string(
    element: &IUIAutomationElement,
    property: windows::Win32::UI::Accessibility::UIA_PROPERTY_ID,
) -> Result<String, UiaError> {
    // SAFETY: property 是封闭的字符串属性，element 留在 worker apartment。
    let value = unsafe { element.GetCurrentPropertyValue(property) }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?;
    BSTR::try_from(&value)
        .map(|value| value.to_string())
        .map_err(|_| UiaError::PropertyTypeMismatch {
            property: super::native::UiaProperty::Value,
        })
}

/// 区分 TreeWalker 的空 parent 与 provider 错误。
fn optional_element(
    result: windows::core::Result<IUIAutomationElement>,
) -> Result<Option<IUIAutomationElement>, UiaError> {
    match result {
        Ok(element) => Ok(Some(element)),
        Err(source) if source.code().0 == 0 => Ok(None),
        Err(source) => Err(UiaError::from_native(UiaOperation::NavigateTree, source)),
    }
}

/// 构造 JSON artifact。
fn json_artifact(
    kind: EvidenceArtifactKind,
    relative_path: &str,
    sensitive: bool,
    value: serde_json::Value,
) -> EvidenceArtifact {
    EvidenceArtifact {
        kind,
        relative_path: PathBuf::from(relative_path),
        sensitive,
        data: EvidenceArtifactData::Json(value),
    }
}

/// 把 UIA 内部错误收敛成 best-effort 采集错误。
fn capture_error(error: UiaError) -> EvidenceCaptureError {
    if matches!(error, UiaError::ExecutionDeadlineExceeded) {
        EvidenceCaptureError::DeadlineExceeded
    } else {
        EvidenceCaptureError::CaptureFailed {
            message: error.to_string(),
        }
    }
}

/// 把 artifact JSON 序列化错误收敛到采集边界。
fn serialize_error(error: serde_json::Error) -> EvidenceCaptureError {
    EvidenceCaptureError::CaptureFailed {
        message: format!("failed to serialize UI Automation snapshot: {error}"),
    }
}
