//! 专用线程的 WH_MOUSE_LL/WH_KEYBOARD_LL。Callback 仅复制固定字段并 try_send。

use crate::{InputPhase, MouseButton, PhysicalEvent, PhysicalInput, RecorderError};

#[cfg(test)]
#[path = "tests/hook_live.rs"]
mod live_tests;
#[cfg(test)]
#[path = "tests/hook_queue.rs"]
mod tests;
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
};
use windows::Win32::{
    Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM},
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::{
        HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
        WindowsAndMessaging::*,
    },
};

/// Hook thread 独占的回调投递状态；没有跨线程可变全局状态或 callback 锁。
struct HookSink {
    wake: Option<Arc<dyn argusflow_capture::CaptureScheduler>>,
    /// 有界队列，满时丢弃并计数，不能阻塞真实输入。
    sender: SyncSender<PhysicalEvent>,
    /// 单调原始序号，丢弃也递增。
    sequence: u64,
    /// 只读状态面板读取累计缺口数。
    dropped: Arc<AtomicU64>,
}

thread_local! {
    /// LL Hook 只回到安装它的线程；TLS 将 callback 所有权限制在该生命周期。
    static SINK: RefCell<Option<HookSink>> = const { RefCell::new(None) };
}

/// Hook 生命周期句柄；Drop 总是尝试卸载并关闭输入源。
pub(crate) struct HookCapture {
    /// 只用于向创建消息队列的线程发送 WM_QUIT。
    thread_id: u32,
    /// Join 发生在停止请求后，不与 callback 分享状态。
    thread: Option<JoinHandle<()>>,
}

impl HookCapture {
    /// 仅反映消息线程是否仍存活，不读取回调状态。
    pub(crate) fn is_running(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }

    /// 安装 hook 并等待安装结果；失败不会留下半安装的键盘/鼠标 hook。
    #[cfg(test)]
    pub(crate) fn start(
        sender: SyncSender<PhysicalEvent>,
        dropped: Arc<AtomicU64>,
    ) -> Result<Self, RecorderError> {
        Self::start_with_wake(sender, dropped, None)
    }

    pub(crate) fn start_with_wake(
        sender: SyncSender<PhysicalEvent>,
        dropped: Arc<AtomicU64>,
        wake: Option<Arc<dyn argusflow_capture::CaptureScheduler>>,
    ) -> Result<Self, RecorderError> {
        let (ready, result) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("argusflow-recorder-hook".into())
            .spawn(move || {
                SINK.with(|sink| {
                    *sink.borrow_mut() = Some(HookSink {
                        wake,
                        sender,
                        sequence: 0,
                        dropped,
                    })
                });
                // SAFETY: 专用线程只服务 Hook，DPI context 随线程退出销毁。
                unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
                let installed = install();
                match installed {
                    Ok((mouse, keyboard)) => {
                        let system = crate::system_events::SystemEventCapture::install();
                        if system.is_err() {
                            unsafe {
                                let _ = UnhookWindowsHookEx(mouse);
                                let _ = UnhookWindowsHookEx(keyboard);
                            }
                            let _ = ready.send(Err(RecorderError::HookUnavailable));
                            SINK.with(|sink| *sink.borrow_mut() = None);
                            return;
                        }
                        // 同一消息线程安装并拥有系统通知，退出时先卸载再关闭输入队列。
                        let _system = system;
                        let mut message = MSG::default();
                        // SAFETY: 先创建线程消息队列，再发布 thread id，避免 stop 的投递竞争。
                        unsafe {
                            let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                        }
                        let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));
                        loop {
                            // SAFETY: message 是有效输出，线程有消息队列。
                            let status = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
                            if status <= 0 {
                                break;
                            }
                            unsafe {
                                let _ = TranslateMessage(&message);
                                DispatchMessageW(&message);
                            }
                        }
                        // SAFETY: 句柄由本线程安装，且每个句柄只卸载一次。
                        unsafe {
                            let _ = UnhookWindowsHookEx(mouse);
                            let _ = UnhookWindowsHookEx(keyboard);
                        }
                    }
                    Err(error) => {
                        let _ = ready.send(Err(error));
                    }
                }
                SINK.with(|sink| *sink.borrow_mut() = None);
            })
            .map_err(|_| RecorderError::HookUnavailable)?;
        match result.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                thread_id,
                thread: Some(thread),
            }),
            _ => {
                let _ = thread.join();
                Err(RecorderError::HookUnavailable)
            }
        }
    }

    /// WM_QUIT 让同一个安装线程先卸载 hook，再释放 sender。
    pub(crate) fn stop(&mut self) -> Result<(), RecorderError> {
        if let Some(thread) = self.thread.take() {
            // SAFETY: thread id 在队列准备完成后才发布，仅发无指针的退出消息。
            if unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }.is_err()
                && !thread.is_finished()
            {
                self.thread = Some(thread);
                return Err(RecorderError::HookUnavailable);
            }
            thread.join().map_err(|_| RecorderError::HookUnavailable)?;
        }
        Ok(())
    }
}

impl Drop for HookCapture {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn install() -> Result<(HHOOK, HHOOK), RecorderError> {
    // SAFETY: callback 代码在当前进程模块中，函数指针在整个 hook 生命周期有效。
    let module = unsafe { GetModuleHandleW(None) }.map_err(|_| RecorderError::HookUnavailable)?;
    let mouse =
        unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), Some(HINSTANCE(module.0)), 0) }
            .map_err(|_| RecorderError::HookUnavailable)?;
    match unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_hook),
            Some(HINSTANCE(module.0)),
            0,
        )
    } {
        Ok(keyboard) => Ok((mouse, keyboard)),
        Err(_) => {
            unsafe {
                let _ = UnhookWindowsHookEx(mouse);
            }
            Err(RecorderError::HookUnavailable)
        }
    }
}

/// Callback 的唯一业务操作：复制固定大小事件并立即非阻塞投递。
pub(crate) fn emit(timestamp_ms: u32, input: PhysicalInput) {
    SINK.with(|sink| {
        if let Ok(mut sink) = sink.try_borrow_mut()
            && let Some(sink) = sink.as_mut()
        {
            sink.sequence += 1;
            if let Some(wake) = &sink.wake {
                wake.wake();
            }
            if sink
                .sender
                .try_send(PhysicalEvent {
                    sequence: sink.sequence,
                    timestamp_ms,
                    input,
                })
                .is_err()
            {
                sink.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    });
}

unsafe extern "system" fn mouse_hook(code: i32, message: WPARAM, parameter: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: HC_ACTION 的 lParam 由 Windows 保证为本次调用有效的 MSLLHOOKSTRUCT。
        let event = unsafe { &*(parameter.0 as *const MSLLHOOKSTRUCT) };
        #[cfg(test)]
        live_tests::observe_mouse(event);
        if event.flags & LLMHF_INJECTED == 0 {
            let point = argusflow_core::ScreenPoint {
                x: event.pt.x,
                y: event.pt.y,
            };
            let input = match message.0 as u32 {
                WM_LBUTTONDOWN => Some(PhysicalInput::Mouse {
                    point,
                    button: MouseButton::Left,
                    phase: InputPhase::Down,
                }),
                WM_LBUTTONUP => Some(PhysicalInput::Mouse {
                    point,
                    button: MouseButton::Left,
                    phase: InputPhase::Up,
                }),
                WM_RBUTTONDOWN => Some(PhysicalInput::Mouse {
                    point,
                    button: MouseButton::Right,
                    phase: InputPhase::Down,
                }),
                WM_RBUTTONUP => Some(PhysicalInput::Mouse {
                    point,
                    button: MouseButton::Right,
                    phase: InputPhase::Up,
                }),
                WM_MBUTTONDOWN => Some(PhysicalInput::Mouse {
                    point,
                    button: MouseButton::Middle,
                    phase: InputPhase::Down,
                }),
                WM_MBUTTONUP => Some(PhysicalInput::Mouse {
                    point,
                    button: MouseButton::Middle,
                    phase: InputPhase::Up,
                }),
                WM_XBUTTONDOWN | WM_XBUTTONUP => match event.mouseData >> 16 {
                    1 | 2 => Some(PhysicalInput::Mouse {
                        point,
                        button: if event.mouseData >> 16 == 1 {
                            MouseButton::X1
                        } else {
                            MouseButton::X2
                        },
                        phase: if message.0 as u32 == WM_XBUTTONDOWN {
                            InputPhase::Down
                        } else {
                            InputPhase::Up
                        },
                    }),
                    _ => None,
                },
                WM_MOUSEMOVE => Some(PhysicalInput::Move { point }),
                WM_MOUSEWHEEL | WM_MOUSEHWHEEL => Some(PhysicalInput::Wheel {
                    point,
                    delta: (event.mouseData >> 16) as i16,
                    horizontal: message.0 as u32 == WM_MOUSEHWHEEL,
                }),
                _ => None,
            };
            if let Some(input) = input {
                emit(event.time, input);
            }
        }
    }
    // SAFETY: 必须保留 hook chain，绝不拦截或修改用户输入。
    unsafe { CallNextHookEx(None, code, message, parameter) }
}

unsafe extern "system" fn keyboard_hook(code: i32, message: WPARAM, parameter: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: HC_ACTION 的结构只在 callback 内借用，队列仅保存值字段。
        let event = unsafe { &*(parameter.0 as *const KBDLLHOOKSTRUCT) };
        #[cfg(test)]
        live_tests::observe_keyboard(event);
        if event.flags.0 & LLKHF_INJECTED.0 == 0 {
            let phase = match message.0 as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => Some(InputPhase::Down),
                WM_KEYUP | WM_SYSKEYUP => Some(InputPhase::Up),
                _ => None,
            };
            if let Some(phase) = phase {
                emit(
                    event.time,
                    PhysicalInput::Key {
                        virtual_key: event.vkCode,
                        scan_code: event.scanCode,
                        flags: event.flags.0,
                        phase,
                    },
                );
            }
        }
    }
    // SAFETY: 始终传递原始参数，录制不得影响输入传播。
    unsafe { CallNextHookEx(None, code, message, parameter) }
}
