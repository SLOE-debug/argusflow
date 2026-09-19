//! 焦点与任务栏命中诊断；仅显式传入 --activate 时测试记事本激活。
use argusflow_windows::{
    OperationOptions, Predicate, Query, SearchScope, UiaRuntime, WindowLocator,
};
use windows::Win32::UI::WindowsAndMessaging::*;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().any(|arg| arg == "--activate") {
        let target = (WindowLocator {
            class_name: Some("Notepad".into()),
            ..Default::default()
        })
        .find_unique()?
        .identity();
        let input = argusflow_windows::InputService::new()?;
        let result = input
            .perform(
                target,
                argusflow_windows::InputAction::ActivateWindow,
                OperationOptions::default(),
            )
            .await;
        input.shutdown(OperationOptions::default()).await?;
        println!("activate_result={result:?}");
    }
    let foreground = unsafe { GetForegroundWindow() };
    let mut pid = 0;
    let tid = unsafe { GetWindowThreadProcessId(foreground, Some(&mut pid)) };
    let mut name = [0u16; 256];
    let length = unsafe { GetWindowTextW(foreground, &mut name) } as usize;
    let mut class = [0u16; 256];
    let class_len = unsafe { GetClassNameW(foreground, &mut class) } as usize;
    println!(
        "foreground={foreground:?} pid={pid} tid={tid} title={} class={}",
        String::from_utf16_lossy(&name[..length]),
        String::from_utf16_lossy(&class[..class_len])
    );
    println!("foreground_exstyle={:#x}", unsafe {
        GetWindowLongPtrW(foreground, GWL_EXSTYLE)
    });
    for window in (WindowLocator {
        class_name: Some("Notepad".into()),
        ..Default::default()
    })
    .find_all()?
    {
        println!("notepad={window:?}");
        unsafe {
            use windows::Win32::Foundation::*;
            use windows::Win32::UI::HiDpi::*;
            let previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let hwnd = HWND(window.identity().handle() as *mut _);
            let mut rect = RECT::default();
            GetWindowRect(hwnd, &mut rect)?;
            println!(
                "rect={rect:?} iconic={:?} visible={:?}",
                IsIconic(hwnd),
                IsWindowVisible(hwnd)
            );
            for fraction in [2, 5, 8] {
                for dy in [12, 28, 44] {
                    let x = rect.left + (rect.right - rect.left) * fraction / 10;
                    let y = rect.top + dy;
                    let hit = WindowFromPoint(POINT { x, y });
                    let mut result = 0;
                    let sent = SendMessageTimeoutW(
                        hwnd,
                        WM_NCHITTEST,
                        WPARAM(0),
                        LPARAM(((x as u32 & 65535) | ((y as u32 & 65535) << 16)) as isize),
                        SMTO_ABORTIFHUNG,
                        100,
                        Some(&mut result),
                    );
                    let mut hp = 0;
                    GetWindowThreadProcessId(hit, Some(&mut hp));
                    let mut hc = [0u16; 128];
                    let hl = GetClassNameW(hit, &mut hc) as usize;
                    println!(
                        "point={x},{y} hit={hit:?} pid={hp} root={:?} class={} ncht={result} sent={sent:?}",
                        GetAncestor(hit, GA_ROOT),
                        String::from_utf16_lossy(&hc[..hl])
                    );
                }
            }
            SetThreadDpiAwarenessContext(previous);
        }
    }
    let runtime = UiaRuntime::start(Default::default(), OperationOptions::default()).await?;
    let taskbar = (WindowLocator {
        class_name: Some("Shell_TrayWnd".into()),
        ..Default::default()
    })
    .find_unique()?
    .identity();
    let button = runtime
        .find_unique(
            Query {
                window: taskbar,
                predicate: Predicate::AutomationId(
                    "Appid: Microsoft.WindowsNotepad_8wekyb3d8bbwe!App".into(),
                ),
                scope: SearchScope::Descendants,
            },
            OperationOptions::default(),
        )
        .await?;
    let snapshot = runtime.read(&button, OperationOptions::default()).await?;
    println!("button={snapshot:?}");
    let [left, top, right, bottom] = snapshot.bounds;
    let observed = runtime
        .observe_target(
            Some([(left + right) / 2, (top + bottom) / 2]),
            OperationOptions::default(),
        )
        .await?;
    println!(
        "hit={:?} ancestors={:?}",
        observed.target, observed.ancestors
    );
    button.release();
    runtime.shutdown(OperationOptions::default()).await?;
    Ok(())
}
