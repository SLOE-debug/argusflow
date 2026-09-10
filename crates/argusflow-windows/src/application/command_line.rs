//! 将独立 argv 编码为 Windows CRT 命令行，永不经过 Shell。
use crate::WindowsError;
use argusflow_core::FailureKind;
use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

pub(super) fn command_line(
    executable: &OsStr,
    arguments: &[String],
) -> Result<Vec<u16>, WindowsError> {
    let mut result = Vec::new();
    for (index, arg) in std::iter::once(executable)
        .chain(arguments.iter().map(OsStr::new))
        .enumerate()
    {
        if index != 0 {
            result.push(u16::from(b' '));
        }
        result.push(u16::from(b'"'));
        let mut slashes = 0;
        for unit in arg.encode_wide() {
            if unit == 0 {
                return Err(WindowsError::new(
                    FailureKind::InvalidInput,
                    "application_args",
                    "进程参数不能包含 NUL",
                ));
            }
            if unit == u16::from(b'\\') {
                slashes += 1;
                continue;
            }
            result.extend(std::iter::repeat_n(
                u16::from(b'\\'),
                if unit == u16::from(b'"') {
                    slashes * 2 + 1
                } else {
                    slashes
                },
            ));
            result.push(unit);
            slashes = 0;
        }
        result.extend(std::iter::repeat_n(u16::from(b'\\'), slashes * 2));
        result.push(u16::from(b'"'));
    }
    result.push(0);
    if result.len() > 32767 {
        return Err(WindowsError::new(
            FailureKind::InvalidInput,
            "application_args",
            "进程命令行超过 Windows 限制",
        ));
    }
    Ok(result)
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/application/command_line.rs"]
mod tests;
