//! 回调只读取小型系统事实并 try_send，不查询 COM、不落盘、不等待。
use super::service::{ListenerState, qpc};
use argusflow_input_contracts::*;
use std::{
    cell::RefCell,
    sync::{Arc, atomic::Ordering, mpsc::SyncSender},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    UI::{Accessibility::HWINEVENTHOOK, WindowsAndMessaging::*},
};
thread_local! {pub(super) static CURRENT: RefCell<Option<Context>>=const{RefCell::new(None)};}
pub(super) struct Context {
    sender: SyncSender<InputEvent>,
    excluded_pid: u32,
    included_pid: Option<u32>,
    state: Arc<ListenerState>,
    sequence: u64,
    window: WindowContext,
    keys: [bool; 256],
    paused: bool,
}
impl Context {
    pub(super) fn new(
        sender: SyncSender<InputEvent>,
        excluded_pid: u32,
        included_pid: Option<u32>,
        state: Arc<ListenerState>,
    ) -> Self {
        Self {
            sender,
            excluded_pid,
            included_pid,
            state,
            sequence: 0,
            window: WindowContext::default(),
            keys: [false; 256],
            paused: false,
        }
    }
    fn emit(
        &mut self,
        mut kind: InputKind,
        point: Point,
        flags: u32,
        extra: usize,
        injected: bool,
        time: u32,
    ) {
        if self.state.stopping.load(Ordering::Acquire) || self.state.lost() > 0 {
            return;
        }
        let paused = self.state.paused.load(Ordering::Acquire);
        if paused != self.paused {
            self.keys.fill(false);
            self.window.epoch += 1;
            self.paused = paused;
        }
        if paused {
            return;
        }
        // SAFETY: 只读当前前台句柄与 PID，不向窗口发消息。
        let foreground = unsafe { GetForegroundWindow() };
        // 鼠标按下到激活窗口之间存在时序差，记录命中根窗口而不是旧前台。
        let hwnd = if matches!(kind, InputKind::Button { .. } | InputKind::Move) {
            unsafe {
                GetAncestor(
                    WindowFromPoint(POINT {
                        x: point.x,
                        y: point.y,
                    }),
                    GA_ROOT,
                )
            }
        } else {
            foreground
        };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if self.window.handle != hwnd.0 as i64 || self.window.pid != pid {
            self.window = WindowContext {
                handle: hwnd.0 as i64,
                pid,
                epoch: self.window.epoch + 1,
            };
            self.keys.fill(false);
        }
        if !super::scope::accepts(kind, pid, self.excluded_pid, self.included_pid) {
            return;
        }
        if let InputKind::Key {
            vk,
            down,
            ref mut repeat,
            ..
        } = kind
            && vk < 256
        {
            *repeat = down && self.keys[vk as usize];
            self.keys[vk as usize] = down;
        }
        self.sequence += 1;
        let origin = if injected && extra == OWN_INPUT_TAG {
            InputOrigin::ArgusFlow
        } else if injected {
            InputOrigin::InjectedUnknown
        } else {
            InputOrigin::System
        };
        let event = InputEvent {
            sequence: self.sequence,
            qpc: qpc(),
            system_time: time,
            point,
            window: self.window,
            foreground_handle: foreground.0 as i64,
            origin,
            flags,
            kind,
        };
        let queued = self.state.queued.fetch_add(1, Ordering::AcqRel) + 1;
        self.state.peak.fetch_max(queued, Ordering::Relaxed);
        if self.sender.try_send(event).is_err() {
            self.state.queued.fetch_sub(1, Ordering::AcqRel);
            self.state.lost.fetch_add(1, Ordering::AcqRel);
        }
    }
}
pub(super) unsafe extern "system" fn mouse(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: Windows supplies MSLLHOOKSTRUCT during WH_MOUSE_LL callback.
        let event = unsafe { &*(l.0 as *const MSLLHOOKSTRUCT) };
        let kind = match w.0 as u32 {
            WM_MOUSEMOVE => Some(InputKind::Move),
            WM_LBUTTONDOWN | WM_LBUTTONUP => Some(InputKind::Button {
                button: Button::Left,
                down: w.0 as u32 == WM_LBUTTONDOWN,
            }),
            WM_RBUTTONDOWN | WM_RBUTTONUP => Some(InputKind::Button {
                button: Button::Right,
                down: w.0 as u32 == WM_RBUTTONDOWN,
            }),
            WM_MBUTTONDOWN | WM_MBUTTONUP => Some(InputKind::Button {
                button: Button::Middle,
                down: w.0 as u32 == WM_MBUTTONDOWN,
            }),
            WM_XBUTTONDOWN | WM_XBUTTONUP => Some(InputKind::Button {
                button: Button::Extra((event.mouseData >> 16) as u16),
                down: w.0 as u32 == WM_XBUTTONDOWN,
            }),
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => Some(InputKind::Wheel {
                horizontal: w.0 as u32 == WM_MOUSEHWHEEL,
                delta: (event.mouseData >> 16) as i16,
            }),
            _ => None,
        };
        if let Some(kind) = kind {
            CURRENT.with(|slot| {
                if let Ok(mut slot) = slot.try_borrow_mut()
                    && let Some(context) = slot.as_mut()
                {
                    context.emit(
                        kind,
                        Point {
                            x: event.pt.x,
                            y: event.pt.y,
                        },
                        event.flags,
                        event.dwExtraInfo,
                        event.flags & LLMHF_INJECTED != 0,
                        event.time,
                    );
                }
            });
        }
    }
    unsafe { CallNextHookEx(None, code, w, l) }
}
pub(super) unsafe extern "system" fn keyboard(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: Windows supplies KBDLLHOOKSTRUCT for this callback.
        let event = unsafe { &*(l.0 as *const KBDLLHOOKSTRUCT) };
        CURRENT.with(|slot| {
            if let Ok(mut slot) = slot.try_borrow_mut()
                && let Some(context) = slot.as_mut()
            {
                let down = matches!(w.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
                let mut point = POINT::default();
                unsafe {
                    let _ = GetCursorPos(&mut point);
                }
                context.emit(
                    InputKind::Key {
                        vk: event.vkCode,
                        scan: event.scanCode,
                        down,
                        repeat: false,
                        extended: event.flags.0 & LLKHF_EXTENDED.0 != 0,
                    },
                    Point {
                        x: point.x,
                        y: point.y,
                    },
                    event.flags.0,
                    event.dwExtraInfo,
                    event.flags.0 & LLKHF_INJECTED.0 != 0,
                    event.time,
                );
            }
        });
    }
    unsafe { CallNextHookEx(None, code, w, l) }
}
pub(super) unsafe extern "system" fn foreground(
    _: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    _: i32,
    _: u32,
    time: u32,
) {
    CURRENT.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut()
            && let Some(context) = slot.as_mut()
        {
            if event == EVENT_OBJECT_DESTROY
                && (object != OBJID_WINDOW.0 || context.window.handle != hwnd.0 as i64)
            {
                return;
            }
            if event == EVENT_OBJECT_DESTROY {
                context.window.epoch += 1;
                context.keys.fill(false);
            }
            let mut point = POINT::default();
            unsafe {
                let _ = GetCursorPos(&mut point);
            }
            context.emit(
                InputKind::Context,
                Point {
                    x: point.x,
                    y: point.y,
                },
                0,
                0,
                false,
                time,
            );
        }
    });
}
