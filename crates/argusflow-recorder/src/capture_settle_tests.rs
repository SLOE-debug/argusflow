//! 使用确定性的绘制时间轴回归通知早于实际像素的场景，不操作用户桌面。
use crate::{InputPhase, MouseButton, PhysicalInput, WindowChange, capture_settle::CaptureSettle};
use argusflow_core::{EvidenceFrame, EvidencePixelFormat, InspectionContext, ScreenPoint};

fn frame(value: u8) -> EvidenceFrame {
    let mut bounds = context().bounds;
    bounds.width = 2.0;
    bounds.height = 2.0;
    EvidenceFrame::new(bounds, 2, 2, EvidencePixelFormat::Rgba8, vec![value; 16]).unwrap()
}

fn context() -> InspectionContext {
    InspectionContext {
        window: argusflow_core::WindowIdentity {
            handle: 42,
            process_id: 7,
        },
        executable_path: None,
        title: "Notepad".into(),
        class_name: "fixture".into(),
        bounds: argusflow_core::InspectionRect {
            x: 0.0,
            y: 0.0,
            width: 2.0,
            height: 2.0,
        },
        browser_viewport: None,
        dpi: 96,
        has_keyboard_focus: true,
    }
}

fn verify_delay(input: PhysicalInput, switch_at: u64, paint_at: u64) {
    let mut history = crate::screen_observer::ScreenHistory::default();
    let baseline = history.push(frame(0), 0, 0);
    let mut settle = CaptureSettle::new(input, Some(baseline.revision));
    for time in (50..=1100).step_by(50) {
        let mut context = context();
        context.has_keyboard_focus = time >= switch_at;
        if matches!(input, PhysicalInput::Key { .. }) && time >= switch_at {
            // Win 触发 Search，前台身份必须随采样刷新。
            context.window.handle = 99;
            context.title = "Search".into();
        }
        let painted = time >= paint_at;
        let sample = history.push(frame(if painted { 255 } else { 0 }), time, 0);
        let done = settle.observe(time, sample.revision);
        if done {
            assert!(time >= paint_at + 150, "不能在绘制前的静止画面结束采样");
            assert_eq!(sample.frame.pixels(), &[255; 16]);
            return;
        }
    }
    panic!("测试时间轴应在预算内稳定");
}

#[test]
fn win_search_uses_new_foreground_after_shell_draws() {
    verify_delay(
        PhysicalInput::Key {
            virtual_key: 0x5B,
            scan_code: 0,
            flags: 0,
            phase: InputPhase::Down,
        },
        800,
        800,
    );
}

#[test]
fn notepad_appeared_waits_for_first_paint() {
    verify_delay(
        PhysicalInput::Window {
            window: context().window,
            change: WindowChange::Appeared,
        },
        800,
        800,
    );
}

#[test]
fn background_click_waits_for_focus_and_result_pixels() {
    verify_delay(
        PhysicalInput::Mouse {
            point: ScreenPoint { x: 0, y: 0 },
            button: MouseButton::Left,
            phase: InputPhase::Down,
        },
        250,
        350,
    );
}

#[test]
fn show_and_foreground_notifications_do_not_prove_render_completion() {
    for change in [WindowChange::Appeared, WindowChange::Foreground] {
        verify_delay(
            PhysicalInput::Window {
                window: context().window,
                change,
            },
            450,
            600,
        );
    }
}

#[test]
fn capture_failure_invalidates_previous_pixels_and_stability() {
    let mut settle = CaptureSettle::new(
        PhysicalInput::Window {
            window: context().window,
            change: WindowChange::Appeared,
        },
        Some(1),
    );
    assert!(!settle.observe(500, 1));
    settle.invalidate();
    assert!(!settle.observe(650, 2));
    assert!(!settle.observe(800, 2));
    assert!(!settle.observe(850, 3));
    assert!(settle.observe(1000, 3));
}
