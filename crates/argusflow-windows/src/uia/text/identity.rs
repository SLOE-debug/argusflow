//! UIA RuntimeId 的有界复制，原生数组不逃逸 COM 线程。
use crate::{WindowsError, platform::failure};
use argusflow_core::FailureKind;
use windows::Win32::{
    System::{Com::SAFEARRAY, Ole::*},
    UI::Accessibility::IUIAutomationElement,
};
struct Array(*mut SAFEARRAY);
impl Drop for Array {
    fn drop(&mut self) {
        // SAFETY: GetRuntimeId 返回的 SAFEARRAY 由当前调用方销毁。
        unsafe {
            let _ = SafeArrayDestroy(self.0);
        }
    }
}
pub(in crate::uia) fn read(element: &IUIAutomationElement) -> Result<Vec<i32>, WindowsError> {
    // SAFETY: COM 调用在所有者线程；逐项复制有界的一维 LONG 数组。
    unsafe {
        let pointer = element
            .GetRuntimeId()
            .map_err(|e| failure("uia_identity", e))?;
        if pointer.is_null() {
            return Ok(Vec::new());
        }
        let array = Array(pointer);
        if SafeArrayGetDim(array.0) != 1 || SafeArrayGetElemsize(array.0) != 4 {
            return Err(WindowsError::new(
                FailureKind::Protocol,
                "uia_identity",
                "RuntimeId 数组类型无效",
            ));
        }
        let lower = SafeArrayGetLBound(array.0, 1).map_err(|e| failure("uia_identity", e))?;
        let upper = SafeArrayGetUBound(array.0, 1).map_err(|e| failure("uia_identity", e))?;
        if i64::from(upper) - i64::from(lower) + 1 > 64 {
            return Err(WindowsError::new(
                FailureKind::ResourceLimit,
                "uia_identity",
                "RuntimeId 超过预算",
            ));
        }
        let mut result = Vec::new();
        for index in lower..=upper {
            let mut value = 0i32;
            SafeArrayGetElement(array.0, &index, (&mut value as *mut i32).cast())
                .map_err(|e| failure("uia_identity", e))?;
            result.push(value);
        }
        Ok(result)
    }
}
