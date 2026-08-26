//! UIA element 的单次运行诊断身份读取。

use std::ffi::c_void;

use windows::Win32::{
    System::{
        Com::SAFEARRAY,
        Ole::{
            SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound,
            SafeArrayGetUBound,
        },
    },
    UI::Accessibility::IUIAutomationElement,
};

use super::error::{UiaError, UiaOperation};

/// 安全读取并拥有 provider 返回的一维整数 SAFEARRAY。
///
/// RuntimeId 只用于单次 materialize 去重和 evidence 关联，绝不能持久化为 selector。
pub(super) fn runtime_id(element: &IUIAutomationElement) -> Result<Vec<i32>, UiaError> {
    // SAFETY: element 留在 UIA worker；返回 SAFEARRAY 立即交给本函数的唯一 guard。
    let array = unsafe { element.GetRuntimeId() }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadRuntimeId, source))?;
    if array.is_null() {
        return Err(UiaError::InvalidRuntimeId);
    }
    let array = SafeArrayGuard(array);
    // SAFETY: guard 持有非空、由 GetRuntimeId 返回且尚未释放的 SAFEARRAY。
    if unsafe { SafeArrayGetDim(array.0) } != 1 {
        return Err(UiaError::InvalidRuntimeId);
    }
    // SAFETY: 已验证数组是一维，维度索引 1 符合 SAFEARRAY API 约定。
    let lower = unsafe { SafeArrayGetLBound(array.0, 1) }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadRuntimeId, source))?;
    // SAFETY: 已验证数组是一维，维度索引 1 符合 SAFEARRAY API 约定。
    let upper = unsafe { SafeArrayGetUBound(array.0, 1) }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadRuntimeId, source))?;
    if upper < lower {
        return Err(UiaError::InvalidRuntimeId);
    }
    let mut id = Vec::with_capacity((upper - lower + 1) as usize);
    for index in lower..=upper {
        let mut value = 0_i32;
        // SAFETY: index 已被上下界约束，输出指针指向有效且独占的 i32。
        unsafe { SafeArrayGetElement(array.0, &index, (&mut value as *mut i32).cast::<c_void>()) }
            .map_err(|source| UiaError::from_native(UiaOperation::ReadRuntimeId, source))?;
        id.push(value);
    }
    Ok(id)
}

/// 确保 runtime id SAFEARRAY 在同一 apartment 内释放。
struct SafeArrayGuard(*mut SAFEARRAY);

impl Drop for SafeArrayGuard {
    fn drop(&mut self) {
        // SAFETY: 指针来自 GetRuntimeId 且只由本 guard 拥有；释放仍发生在 UIA worker。
        let _ = unsafe { SafeArrayDestroy(self.0) };
    }
}
