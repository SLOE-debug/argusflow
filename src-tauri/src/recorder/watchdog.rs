//! 独立于日志 I/O 的父进程租约，磁盘卡住也不会留下无限录制进程。
use argusflow_windows::listening::{ListenerState, qpc};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI64, Ordering},
};
pub(super) struct Watchdog {
    last: Arc<AtomicI64>,
    done: Arc<AtomicBool>,
}
impl Watchdog {
    pub(super) fn start(state: Arc<ListenerState>, frequency: u64, alive: Arc<AtomicBool>) -> Self {
        let last = Arc::new(AtomicI64::new(qpc()));
        let seen = last.clone();
        let done = Arc::new(AtomicBool::new(false));
        let finished = done.clone();
        std::thread::spawn(move || {
            while !finished.load(Ordering::Acquire) {
                if !alive.load(Ordering::Acquire)
                    || qpc().saturating_sub(seen.load(Ordering::Acquire)) as u128
                        > u128::from(frequency) * 3
                {
                    state.stop();
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    if !finished.load(Ordering::Acquire) {
                        std::process::exit(2);
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        });
        Self { last, done }
    }
    pub(super) fn heartbeat(&self) -> Arc<AtomicI64> {
        self.last.clone()
    }
}
impl Drop for Watchdog {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Release);
    }
}
