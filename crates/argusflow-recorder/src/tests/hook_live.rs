//! 显式运行的 Windows 桌面测试。真实 SetWindowsHookEx；不在常规 CI 自动监听。

use super::*;
use std::{
    sync::atomic::AtomicUsize,
    time::{Duration, Instant},
};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

/// 仅识别测试自己产生的注入事件，不统计其他程序的注入。
const MARKER: usize = 0x41524755;
/// 人工指定注入时间，队列过滤断言不会误认同时发生的用户输入。
const EVENT_TIME: u32 = 0x10203040;
static INJECTED_MOUSE: AtomicUsize = AtomicUsize::new(0);
static INJECTED_KEYBOARD: AtomicUsize = AtomicUsize::new(0);
/// 仅诊断测试期间 Windows 标记为注入的鼠标事件数量。
static OTHER_INJECTED_MOUSE: AtomicUsize = AtomicUsize::new(0);
/// 只序列化两个需要真实桌面的用例，普通纯数据测试不参与此锁。
static DESKTOP_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(super) fn observe_mouse(event: &MSLLHOOKSTRUCT) {
    if event.dwExtraInfo != MARKER && event.flags & LLMHF_INJECTED != 0 {
        OTHER_INJECTED_MOUSE.fetch_add(1, Ordering::Relaxed);
    }
    if event.dwExtraInfo == MARKER && event.flags & LLMHF_INJECTED != 0 {
        INJECTED_MOUSE.fetch_add(1, Ordering::Relaxed);
    }
}

pub(super) fn observe_keyboard(event: &KBDLLHOOKSTRUCT) {
    if event.dwExtraInfo == MARKER && event.flags.0 & LLKHF_INJECTED.0 != 0 {
        INJECTED_KEYBOARD.fetch_add(1, Ordering::Relaxed);
    }
}

/// F24 与往返一像素 mouse move 不输入文字、不点击控件；标记由 Windows 添加。
fn send_probe() {
    let inputs = [
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 1,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_MOVE_NOCOALESCE,
                    time: EVENT_TIME,
                    dwExtraInfo: MARKER,
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: -1,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_MOVE_NOCOALESCE,
                    time: EVENT_TIME,
                    dwExtraInfo: MARKER,
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_F24,
                    time: EVENT_TIME,
                    dwExtraInfo: MARKER,
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_F24,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: EVENT_TIME,
                    dwExtraInfo: MARKER,
                    ..Default::default()
                },
            },
        },
    ];
    // SAFETY: 数组与 INPUT 大小匹配；按下/释放成对，不产生应用文本或点击。
    assert_eq!(
        unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) },
        4
    );
}

#[test]
#[ignore = "installs real global hooks and sends harmless tagged F24/mouse input on Windows"]
fn windows_global_hooks_receive_filter_stop_and_restart() {
    let _desktop = DESKTOP_TEST.lock().unwrap();
    for cycle in 1..=3 {
        INJECTED_MOUSE.store(0, Ordering::Relaxed);
        INJECTED_KEYBOARD.store(0, Ordering::Relaxed);
        let (sender, receiver) = mpsc::sync_channel(4096);
        let dropped = Arc::new(AtomicU64::new(0));
        let mut hook = HookCapture::start(sender, dropped.clone()).unwrap();
        assert!(hook.is_running());
        send_probe();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline
            && (INJECTED_MOUSE.load(Ordering::Relaxed) < 2
                || INJECTED_KEYBOARD.load(Ordering::Relaxed) < 2)
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        println!(
            "callback counts: mouse={}, keyboard={}",
            INJECTED_MOUSE.load(Ordering::Relaxed),
            INJECTED_KEYBOARD.load(Ordering::Relaxed)
        );
        assert_eq!(
            INJECTED_MOUSE.load(Ordering::Relaxed),
            2,
            "Windows mouse callback"
        );
        assert_eq!(
            INJECTED_KEYBOARD.load(Ordering::Relaxed),
            2,
            "Windows key down/up callbacks"
        );
        hook.stop().unwrap();
        assert!(!hook.is_running());
        // 接收器只检查标记时间，不读取或打印同时发生的用户输入内容。
        for event in receiver.try_iter() {
            assert_ne!(event.timestamp_ms, EVENT_TIME);
        }
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
        // Hook 已卸载，同样系统输入不再进入本测试 callback。
        send_probe();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(INJECTED_MOUSE.load(Ordering::Relaxed), 2);
        assert_eq!(INJECTED_KEYBOARD.load(Ordering::Relaxed), 2);
        assert_eq!(dropped.load(Ordering::Relaxed), 0);
        println!("cycle={cycle}: real mouse=2 keyboard=2, injected filtered, stopped and detached");
    }
}

#[test]
#[ignore = "requires physical mouse click and keyboard input within 60 seconds; logs only counts"]
fn windows_global_hooks_capture_physical_input() {
    let _desktop = DESKTOP_TEST.lock().unwrap();
    OTHER_INJECTED_MOUSE.store(0, Ordering::Relaxed);
    let (sender, receiver) = mpsc::sync_channel(4096);
    let dropped = Arc::new(AtomicU64::new(0));
    let mut hook = HookCapture::start(sender, dropped.clone()).unwrap();
    println!(
        "PHYSICAL HOOK READY: click once and press/release a key within 60s; only event counts are retained."
    );
    let deadline = Instant::now() + Duration::from_secs(60);
    let (mut mouse_down, mut mouse_up, mut key_down, mut key_up) = (0, 0, 0, 0);
    let mut previous = 0;
    while Instant::now() < deadline {
        if let Ok(event) = receiver.recv_timeout(Duration::from_millis(100)) {
            assert!(event.sequence > previous);
            previous = event.sequence;
            match event.input {
                PhysicalInput::Mouse {
                    phase: InputPhase::Down,
                    ..
                } => mouse_down += 1,
                PhysicalInput::Mouse {
                    phase: InputPhase::Up,
                    ..
                } => mouse_up += 1,
                PhysicalInput::Key {
                    phase: InputPhase::Down,
                    ..
                } => key_down += 1,
                PhysicalInput::Key {
                    phase: InputPhase::Up,
                    ..
                } => key_up += 1,
                _ => {}
            }
        }
        if mouse_down > 0 && mouse_up > 0 && key_down > 0 && key_up > 0 {
            break;
        }
    }
    hook.stop().unwrap();
    println!(
        "other injected mouse callbacks={}",
        OTHER_INJECTED_MOUSE.load(Ordering::Relaxed)
    );
    println!(
        "PHYSICAL HOOK STOPPED: mouse_down={mouse_down}, mouse_up={mouse_up}, key_down={key_down}, key_up={key_up}, dropped={}",
        dropped.load(Ordering::Relaxed)
    );
    assert!(
        mouse_down > 0 && mouse_up > 0,
        "No physical mouse click observed"
    );
    assert!(key_down > 0 && key_up > 0, "No physical key pair observed");
    assert_eq!(dropped.load(Ordering::Relaxed), 0);
}
