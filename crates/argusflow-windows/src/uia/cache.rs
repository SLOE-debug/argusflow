//! residual 属性的精确 UIA CacheRequest 物化。

use windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationCacheRequest, TreeScope_Element,
};

use super::{
    condition::property_id,
    error::{UiaError, UiaOperation},
    native::UiaPropertyProjection,
};

/// 创建只缓存 compiler 声明投影的 Control View request。
pub(crate) fn build_cache_request(
    automation: &IUIAutomation,
    projections: &[UiaPropertyProjection],
) -> Result<IUIAutomationCacheRequest, UiaError> {
    // SAFETY: automation client 只由当前 UIA worker apartment 调用。
    let request = unsafe { automation.CreateCacheRequest() }
        .map_err(|source| UiaError::from_native(UiaOperation::BuildCache, source))?;
    for projection in projections {
        // SAFETY: property id 来自封闭的 UiaProperty 映射，request 仍在当前 apartment。
        unsafe { request.AddProperty(property_id(projection.property())) }
            .map_err(|source| UiaError::from_native(UiaOperation::BuildCache, source))?;
    }
    // SAFETY: TreeScope_Element 是 CacheRequest 接受的有效原生枚举值。
    unsafe { request.SetTreeScope(TreeScope_Element) }
        .map_err(|source| UiaError::from_native(UiaOperation::BuildCache, source))?;
    // SAFETY: automation client 只由当前 UIA worker apartment 调用。
    let control_view = unsafe { automation.ControlViewCondition() }
        .map_err(|source| UiaError::from_native(UiaOperation::BuildCache, source))?;
    // SAFETY: condition 和 request 均由同一个 automation client/apartment 创建。
    unsafe { request.SetTreeFilter(&control_view) }
        .map_err(|source| UiaError::from_native(UiaOperation::BuildCache, source))?;
    Ok(request)
}
