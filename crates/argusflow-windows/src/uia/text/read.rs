//! 所有 COM 范围留在 UIA MTA 中；只移动克隆范围的端点。
use super::{UiaTextObservation, UiaTextRange, UiaTextSnapshot};
use crate::{WindowsError, platform::failure};
use argusflow_core::Operation;
use windows::Win32::UI::Accessibility::*;

const TEXT_LIMIT: usize = 8192;
const PREFIX_LIMIT: usize = 65536;
const RANGE_LIMIT: i32 = 16;

pub(in crate::uia) fn observe(
    element: &IUIAutomationElement,
    password: bool,
    operation: &Operation,
) -> UiaTextObservation {
    if password {
        return UiaTextObservation::Sensitive;
    }
    match read(element, operation) {
        Ok(Some(value)) => UiaTextObservation::Available(value),
        Ok(None) => UiaTextObservation::Unsupported,
        Err(error) => UiaTextObservation::Unavailable(error.to_string()),
    }
}

fn read(
    element: &IUIAutomationElement,
    operation: &Operation,
) -> Result<Option<UiaTextSnapshot>, WindowsError> {
    // SAFETY: 仅由 UIA 所属 MTA 调用；COM 范围不向调用方暴露。
    unsafe {
        let pattern =
            match element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
                Ok(value) => value,
                Err(e)
                    if e.code() == windows::Win32::Foundation::E_NOINTERFACE
                        || e.code().is_ok()
                        || e.code().0 as u32 == UIA_E_NOTSUPPORTED =>
                {
                    return Ok(None);
                }
                Err(e) => return Err(failure("uia_text_pattern", e)),
            };
        operation.check("uia_text_document")?;
        let document = pattern
            .DocumentRange()
            .map_err(|e| failure("uia_text_document", e))?;
        let (text, document_truncated) = bounded_text(&document, TEXT_LIMIT)?;
        let editable =
            match element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) {
                Ok(value) => Some(
                    !value
                        .CurrentIsReadOnly()
                        .map_err(|e| failure("uia_text_readonly", e))?
                        .as_bool(),
                ),
                Err(e)
                    if e.code() == windows::Win32::Foundation::E_NOINTERFACE
                        || e.code().is_ok()
                        || e.code().0 as u32 == UIA_E_NOTSUPPORTED =>
                {
                    None
                }
                Err(e) => return Err(failure("uia_text_readonly", e)),
            };
        let ranges = pattern
            .GetSelection()
            .map_err(|e| failure("uia_text_selection", e))?;
        let length = ranges
            .Length()
            .map_err(|e| failure("uia_text_selection", e))?;
        let mut selections = Vec::new();
        for index in 0..length.min(RANGE_LIMIT) {
            operation.check("uia_text_range")?;
            let range = ranges
                .GetElement(index)
                .map_err(|e| failure("uia_text_range", e))?;
            let collapsed = range
                .CompareEndpoints(
                    TextPatternRangeEndpoint_Start,
                    &range,
                    TextPatternRangeEndpoint_End,
                )
                .map_err(|e| failure("uia_text_endpoints", e))?
                == 0;
            let (selected, truncated) = bounded_text(&range, TEXT_LIMIT)?;
            let prefix = document.Clone().map_err(|e| failure("uia_text_clone", e))?;
            prefix
                .MoveEndpointByRange(
                    TextPatternRangeEndpoint_End,
                    &range,
                    TextPatternRangeEndpoint_Start,
                )
                .map_err(|e| failure("uia_text_prefix", e))?;
            let (prefix, prefix_truncated) = bounded_text(&prefix, PREFIX_LIMIT)?;
            let start_utf16 = (!prefix_truncated).then(|| prefix.encode_utf16().count() as u32);
            let end_utf16 = start_utf16
                .filter(|_| !truncated)
                .map(|start| start + selected.encode_utf16().count() as u32);
            selections.push(UiaTextRange {
                collapsed,
                text: selected,
                truncated,
                start_utf16,
                end_utf16,
            });
        }
        operation.check("uia_text_complete")?;
        Ok(Some(UiaTextSnapshot {
            document: text,
            document_truncated,
            editable,
            truncated: length > RANGE_LIMIT
                || selections
                    .iter()
                    .any(|s| s.truncated || s.start_utf16.is_none()),
            selections,
        }))
    }
}

fn bounded_text(
    range: &IUIAutomationTextRange,
    limit: usize,
) -> Result<(String, bool), WindowsError> {
    // SAFETY: range 属于当前 MTA；读取上限含一个探测单元，不请求无限全文。
    let value =
        unsafe { range.GetText((limit + 1) as i32) }.map_err(|e| failure("uia_text_read", e))?;
    let units: &[u16] = &value;
    let truncated = units.len() > limit;
    let mut end = units.len().min(limit);
    // 不在 UTF-16 代理对中间截断。
    if truncated && end > 0 && (0xD800..=0xDBFF).contains(&units[end - 1]) {
        end -= 1;
    }
    Ok((String::from_utf16_lossy(&units[..end]), truncated))
}
