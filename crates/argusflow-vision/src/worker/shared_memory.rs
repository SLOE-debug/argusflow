//! OCR ROI 的 Windows Pagefile-backed 共享内存租约。

use std::{
    os::windows::io::{FromRawHandle, OwnedHandle},
    ptr,
};

use windows::{
    Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::Memory::{
            CreateFileMappingW, FILE_MAP_WRITE, MapViewOfFile, PAGE_READWRITE, UnmapViewOfFile,
        },
    },
    core::PCWSTR,
};

use crate::VisionError;

/// 在 OCR 响应返回前保持 mapping 存活的独占租约。
#[derive(Debug)]
pub(super) struct SharedMemoryLease {
    /// Worker 使用的当前登录会话 mapping 名称。
    name: String,
    /// 关闭后由系统回收 Pagefile-backed mapping。
    _handle: OwnedHandle,
}

impl SharedMemoryLease {
    /// 创建 mapping、复制一次只读像素，然后立即解除 Rust 侧视图。
    pub(super) fn create(pixels: &[u8]) -> Result<Self, VisionError> {
        if pixels.is_empty() {
            return Err(VisionError::Protocol {
                message: "shared-memory OCR pixels must not be empty".to_owned(),
            });
        }
        let lease_id = uuid::Uuid::new_v4();
        let name = format!(r"Local\argusflow-vision-{lease_id}");
        let wide_name = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let length = u64::try_from(pixels.len()).map_err(|_| VisionError::Protocol {
            message: "shared-memory OCR pixels exceed u64".to_owned(),
        })?;
        let high = u32::try_from(length >> 32).expect("high mapping length is bounded to u32");
        let low = length as u32;
        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                high,
                low,
                PCWSTR(wide_name.as_ptr()),
            )
        }
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to create OCR shared memory: {error}"),
        })?;
        let owned = unsafe { OwnedHandle::from_raw_handle(handle.0) };
        let view = unsafe { MapViewOfFile(handle, FILE_MAP_WRITE, 0, 0, pixels.len()) };
        if view.Value.is_null() {
            return Err(VisionError::Protocol {
                message: "failed to map OCR shared memory for writing".to_owned(),
            });
        }
        unsafe {
            ptr::copy_nonoverlapping(pixels.as_ptr(), view.Value.cast::<u8>(), pixels.len());
        }
        let unmap_result = unsafe { UnmapViewOfFile(view) };
        unmap_result.map_err(|error| VisionError::Protocol {
            message: format!("failed to unmap OCR shared memory: {error}"),
        })?;
        Ok(Self {
            name,
            _handle: owned,
        })
    }

    /// 返回 Python 用于打开同一 mapping 的名称。
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    /// 返回协议关联使用的租约 ID。
    pub(super) fn lease_id(&self) -> &str {
        self.name
            .rsplit_once('-')
            .map_or(self.name.as_str(), |(_, lease_id)| lease_id)
    }
}
