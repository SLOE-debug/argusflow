//! 仅对本测试创建的临时目录设置 ACL，退出时恢复写权限。
use std::{io, os::windows::process::CommandExt, path::Path, process::Command};

pub struct WriteDeniedDirectory {
    directory: tempfile::TempDir,
    restored: bool,
}

impl WriteDeniedDirectory {
    pub fn new() -> io::Result<Self> {
        let mut fixture = Self {
            directory: tempfile::tempdir()?,
            restored: false,
        };
        // Everyone SID 不依赖系统语言；不继承到父目录，也不操作真实 AppData。
        // 仅拒绝创建文件，保留读取目录和 ACL 的能力。
        fixture.acl(&["/deny", "*S-1-1-0:(WD)"])?;
        fixture.restored = false;
        Ok(fixture)
    }

    pub fn path(&self) -> &Path {
        self.directory.path()
    }

    pub fn restore(&mut self) -> io::Result<()> {
        self.acl(&["/remove:d", "*S-1-1-0"])?;
        self.restored = true;
        Ok(())
    }

    fn acl(&self, arguments: &[&str]) -> io::Result<()> {
        let output = Command::new("icacls.exe")
            .arg(self.directory.path())
            .args(arguments)
            // CREATE_NO_WINDOW：验收工具不弹出控制台。
            .creation_flags(0x0800_0000)
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "测试目录 ACL 操作失败：{}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }
}

impl Drop for WriteDeniedDirectory {
    fn drop(&mut self) {
        if !self.restored
            && let Err(error) = self.restore()
        {
            eprintln!(
                "无法恢复测试目录 ACL {}：{error}",
                self.directory.path().display()
            );
        }
    }
}
