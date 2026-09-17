//! Hook 所属消息线程及有界通道；日志写者不进入消息线程。
use super::callbacks::{self, Context};
use argusflow_input_contracts::InputEvent;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::SyncSender,
    },
    time::Duration,
};
use windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
use windows::Win32::{
    System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
    UI::{Accessibility::*, WindowsAndMessaging::*},
};

/// 可观察的队列故障和峰值；溢出立即停止健康输入。
#[derive(Default)]
pub struct ListenerState {
    pub(super) paused: AtomicBool,
    paused_applied: AtomicBool,
    finished: AtomicBool,
    pub(super) stopping: AtomicBool,
    pub(super) lost: AtomicU64,
    pub(super) queued: AtomicUsize,
    pub(super) peak: AtomicUsize,
}
impl ListenerState {
    /// 已知不能提交的输入数量。
    pub fn lost(&self) -> u64 {
        self.lost.load(Ordering::Acquire)
    }
    /// 原始队列峰值。
    pub fn peak(&self) -> usize {
        self.peak.load(Ordering::Acquire)
    }
    /// 写者收到一条时归还队列统计。
    pub fn consumed(&self) {
        self.queued.fetch_sub(1, Ordering::AcqRel);
    }
    /// 暂停期间回调不提交输入。
    pub fn pause(&self, paused: bool) {
        self.paused.store(paused, Ordering::Release);
    }
    /// 暂停控制线程等待消息线程确认，保证边界前的回调已经提交完毕。
    pub fn wait_paused(&self) -> Result<(), String> {
        self.pause(true);
        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        while !self.paused_applied.load(Ordering::Acquire) {
            if self.finished() || std::time::Instant::now() >= deadline {
                return Err("监听暂停未确认".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    }
    /// 原生消息线程已经退出。
    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
    /// 不再接受输入；消息线程有界退出。
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
    }
}
/// Hook 线程所有者；Drop 只请求停止，原生线程自己释放句柄。
pub struct InputListener {
    state: Arc<ListenerState>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl InputListener {
    /// 启动只读全桌面监听；返回真实就绪握手。
    pub fn start(sender: SyncSender<InputEvent>, excluded_pid: u32) -> Result<Self, String> {
        Self::start_scoped(sender, excluded_pid, None)
    }
    /// 可限制为一个显式进程；桌面录制传 None，原生验收仅使用专属窗口进程。
    pub fn start_scoped(
        sender: SyncSender<InputEvent>,
        excluded_pid: u32,
        included_pid: Option<u32>,
    ) -> Result<Self, String> {
        let state = Arc::new(ListenerState::default());
        let shared = state.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("argusflow-input-listener".into())
            .spawn(move || {
                let result = run(
                    sender,
                    excluded_pid,
                    included_pid,
                    shared.clone(),
                    ready_tx.clone(),
                );
                shared.finished.store(true, Ordering::Release);
                if let Err(error) = result {
                    let _ = ready_tx.try_send(Err(error));
                }
            })
            .map_err(|e| e.to_string())?;
        let listener = Self {
            state,
            thread: Some(thread),
        };
        ready_rx
            .recv_timeout(Duration::from_secs(3))
            .map_err(|e| e.to_string())??;
        Ok(listener)
    }
    /// 共享只读诊断与暂停／停止控制。
    pub fn state(&self) -> Arc<ListenerState> {
        self.state.clone()
    }
    /// 等待 Hook 实际卸载，不等待任何结构或图像服务。
    pub fn shutdown(&mut self) -> Result<(), String> {
        self.state.stop();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while self.thread.as_ref().is_some_and(|t| !t.is_finished()) {
            if std::time::Instant::now() >= deadline {
                return Err("输入监听线程未退出".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| "输入监听线程异常".to_string())?;
        }
        Ok(())
    }
}
impl Drop for InputListener {
    fn drop(&mut self) {
        self.state.stop();
    }
}

struct Hooks {
    mouse: HHOOK,
    keyboard: HHOOK,
    foreground: HWINEVENTHOOK,
    destroy: HWINEVENTHOOK,
}
impl Drop for Hooks {
    fn drop(&mut self) {
        // SAFETY: 句柄仅由当前消息线程创建并释放，回调随卸载停止。
        unsafe {
            let _ = UnhookWindowsHookEx(self.mouse);
            let _ = UnhookWindowsHookEx(self.keyboard);
            let _ = UnhookWinEvent(self.foreground);
            let _ = UnhookWinEvent(self.destroy);
        }
    }
}
fn run(
    sender: SyncSender<InputEvent>,
    excluded_pid: u32,
    included_pid: Option<u32>,
    state: Arc<ListenerState>,
    ready: SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let _dpi = crate::platform::PhysicalDpi::enter().map_err(|e| e.to_string())?;
    callbacks::CURRENT.with(|slot| {
        *slot.borrow_mut() = Some(Context::new(
            sender,
            excluded_pid,
            included_pid,
            state.clone(),
        ))
    });
    // SAFETY: 低层回调在安装线程执行，进程存活期间函数地址有效。
    let mouse = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(callbacks::mouse), None, 0) }
        .map_err(|e| e.to_string())?;
    let keyboard =
        match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(callbacks::keyboard), None, 0) } {
            Ok(hook) => hook,
            Err(error) => {
                unsafe {
                    let _ = UnhookWindowsHookEx(mouse);
                }
                return Err(error.to_string());
            }
        };
    let foreground = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(callbacks::foreground),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    let destroy = unsafe {
        SetWinEventHook(
            EVENT_OBJECT_DESTROY,
            EVENT_OBJECT_DESTROY,
            None,
            Some(callbacks::foreground),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    let _hooks = Hooks {
        mouse,
        keyboard,
        foreground,
        destroy,
    };
    if foreground.is_invalid() || destroy.is_invalid() {
        return Err("前台窗口事件监听安装失败".into());
    }
    ready.send(Ok(())).map_err(|e| e.to_string())?;
    let mut message = MSG::default();
    while !state.stopping.load(Ordering::Acquire) {
        state
            .paused_applied
            .store(state.paused.load(Ordering::Acquire), Ordering::Release);
        // SAFETY: 只调度本线程消息，不操作用户窗口。
        unsafe {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    callbacks::CURRENT.with(|slot| slot.borrow_mut().take());
    Ok(())
}
/// 原始高精度计数器；Windows 10/11 支持该计数器。
pub fn qpc() -> i64 {
    let mut value = 0;
    unsafe {
        let _ = QueryPerformanceCounter(&mut value);
    }
    value
}
/// QPC 每秒频率。
pub fn qpc_frequency() -> Result<u64, String> {
    let mut value = 0;
    unsafe { QueryPerformanceFrequency(&mut value) }.map_err(|e| e.to_string())?;
    u64::try_from(value).map_err(|e| e.to_string())
}
/// 系统双击时间毫秒及物理距离预算。
pub fn gesture_settings() -> (u32, i32) {
    unsafe {
        (
            GetDoubleClickTime(),
            GetSystemMetrics(SM_CXDOUBLECLK)
                .max(GetSystemMetrics(SM_CYDOUBLECLK))
                .max(1),
        )
    }
}
