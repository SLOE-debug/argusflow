//! 测试专用进程树，仅经随机本地端口报告自有子进程 PID。
use std::{
    io::Write,
    process::{Child, Command},
    time::Duration,
};

struct Descendant(Child);
impl Drop for Descendant {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub fn run() {
    let args = std::env::args().collect::<Vec<_>>();
    let mut child = if args.get(1).is_some_and(|arg| arg == "--report") {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.arg("--child");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let child = Descendant(command.spawn().unwrap());
        let mut stream = std::net::TcpStream::connect(&args[2]).unwrap();
        stream.write_all(&child.0.id().to_le_bytes()).unwrap();
        Some(child)
    } else {
        None
    };
    loop {
        if let Some(child) = &mut child {
            let _ = child.0.try_wait();
        }
        std::thread::park_timeout(Duration::from_secs(60));
    }
}
