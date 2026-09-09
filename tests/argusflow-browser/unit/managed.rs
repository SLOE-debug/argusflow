//! 使用本测试程序作为子进程，不启动任何浏览器。
use crate::process::{Managed, reserve};
use argusflow_core::{Operation, OperationOptions};

#[test]
fn process_fixture() {
    if std::env::var_os("ARGUSFLOW_PROCESS_FIXTURE").is_some() {
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}

#[tokio::test]
async fn owned_process_and_profile_are_reaped() {
    let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "tests::managed::process_fixture", "--nocapture"])
        .env("ARGUSFLOW_PROCESS_FIXTURE", "1")
        .kill_on_drop(true)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW，测试辅助进程不创建控制台。
    let child = command.spawn().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let path = profile.path().to_owned();
    std::fs::write(path.join("owned-file"), b"fixture").unwrap();
    let mut managed = Managed::new(child, profile, reserve().unwrap()).unwrap();
    managed
        .reap(&Operation::new(OperationOptions::default()))
        .await
        .unwrap();
    assert!(managed.child.is_none());
    assert!(!path.exists());
}
