//! 只监听专属测试进程，不采集其他应用输入。
use super::*;
use argusflow_input_contracts::{InputKind, InputOrigin};
use argusflow_windows::listening::InputListener;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "仅操作专属Win32验收窗口，短暂使用光标与焦点"]
async fn listener_and_point_observation_preserve_native_facts() {
    let _dpi = support::DpiContext::physical();
    let fixture = support::Fixture::create();
    unsafe {
        let _ = SetForegroundWindow(fixture.hwnd());
    }
    let window = WindowIdentity::from_handle(fixture.window).unwrap();
    window.require_foreground().unwrap();
    let (sender, receiver) = std::sync::mpsc::sync_channel(128);
    let mut listener = InputListener::start_scoped(sender, 0, Some(std::process::id())).unwrap();
    let runtime = UiaRuntime::start(Default::default(), options())
        .await
        .unwrap();
    let input = InputService::new().unwrap();
    let mut point = POINT { x: 80, y: 40 };
    unsafe { ClientToScreen(fixture.hwnd(), &mut point) }
        .ok()
        .unwrap();
    let observed = runtime
        .observe_target(Some([point.x, point.y]), options())
        .await
        .unwrap();
    assert_eq!(observed.target.name, "Invoke target");
    assert_eq!(observed.target.pid as u32, std::process::id());
    let mut latency = vec![];
    for _ in 0..20 {
        let started = std::time::Instant::now();
        let target = runtime
            .observe_target(Some([point.x, point.y]), options())
            .await
            .unwrap();
        latency.push(started.elapsed().as_micros() as u64);
        assert_eq!(target.target.name, "Invoke target");
    }
    latency.sort_unstable();
    println!(
        "RECORDER_UIA samples=20 p50_us={} p95_us={} p99_us={} max_us={}",
        latency[9], latency[18], latency[19], latency[19]
    );
    input
        .perform(
            window.clone(),
            InputAction::Click {
                point: ScreenPoint {
                    x: point.x,
                    y: point.y,
                },
                button: MouseButton::Left,
                count: ClickCount::Single,
            },
            options(),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let events: Vec<_> = receiver.try_iter().collect();
    for _ in &events {
        listener.state().consumed();
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e.kind, InputKind::Button { down: true, .. })
                && e.origin == InputOrigin::ArgusFlow)
    );
    assert!(
        events
            .windows(2)
            .all(|w| w[0].sequence < w[1].sequence && w[0].qpc <= w[1].qpc)
    );
    assert!(events.iter().all(|e| e.window.pid == std::process::id()));
    listener.state().wait_paused().unwrap();
    while receiver.try_recv().is_ok() {
        listener.state().consumed();
    }
    input
        .perform(
            window.clone(),
            InputAction::Move(ScreenPoint {
                x: point.x + 1,
                y: point.y,
            }),
            options(),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(receiver.try_recv().is_err());
    assert_eq!(listener.state().lost(), 0);
    listener.state().pause(false);
    input
        .perform(
            window.clone(),
            InputAction::Move(ScreenPoint {
                x: point.x + 2,
                y: point.y,
            }),
            options(),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(receiver.try_recv().is_ok());
    println!(
        "RECORDER_HOOK queue_peak={} known_lost={}",
        listener.state().peak(),
        listener.state().lost()
    );
    listener.shutdown().unwrap();
    // 不消费容量1的测试队列，真实Hook必须及时返回并留下明确溢出状态。
    let (tx, _rx) = std::sync::mpsc::sync_channel(1);
    let mut overloaded = InputListener::start_scoped(tx, 0, Some(std::process::id())).unwrap();
    for offset in 3..8 {
        input
            .perform(
                window.clone(),
                InputAction::Move(ScreenPoint {
                    x: point.x + offset,
                    y: point.y,
                }),
                options(),
            )
            .await
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(overloaded.state().lost() > 0);
    overloaded.shutdown().unwrap();
    input.shutdown(options()).await.unwrap();
    runtime.shutdown(options()).await.unwrap();
}
