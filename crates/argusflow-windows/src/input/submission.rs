//! 可控输入适配边界：检查注入数量，只补偿已经按下而未释放的事件。
use crate::WindowsError as Failure;
use argusflow_core::FailureKind;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub(super) fn submit(events: &[INPUT]) -> Result<(), Failure> {
    submit_with(events, |events| {
        // SAFETY: INPUT 数组和 ABI 尺寸匹配；GetLastError 必须先于补偿调用采集。
        let count = unsafe { SendInput(events, std::mem::size_of::<INPUT>() as i32) } as usize;
        (count, windows::core::Error::from_thread())
    })
}
fn submit_with(
    events: &[INPUT],
    mut inject: impl FnMut(&[INPUT]) -> (usize, windows::core::Error),
) -> Result<(), Failure> {
    let (inserted, source) = inject(events);
    if inserted == events.len() {
        return Ok(());
    }
    let releases = pending_releases(&events[..inserted.min(events.len())]);
    let cleaned = if releases.is_empty() {
        0
    } else {
        inject(&releases).0
    };
    Err(Failure::new(
        FailureKind::Native,
        "send_input",
        format!(
            "仅注入 {inserted}/{} 个事件；按键释放 {cleaned}/{}，可能受 UIPI 限制",
            events.len(),
            releases.len()
        ),
    )
    .with_source(source))
}
fn pending_releases(events: &[INPUT]) -> Vec<INPUT> {
    let mut pressed = Vec::<((u32, u16, u16), INPUT)>::new();
    for event in events {
        // SAFETY: 按 r#type 检查后读取 INPUT 的对应 union 字段。
        let (key, down, release) = unsafe {
            if event.r#type == INPUT_KEYBOARD {
                let key = event.Anonymous.ki;
                let mut release = *event;
                release.Anonymous.ki.dwFlags |= KEYEVENTF_KEYUP;
                (
                    (0, key.wVk.0, key.wScan),
                    !key.dwFlags.contains(KEYEVENTF_KEYUP),
                    release,
                )
            } else if event.r#type == INPUT_MOUSE {
                let flags = event.Anonymous.mi.dwFlags;
                let (code, down, up) =
                    if flags.contains(MOUSEEVENTF_LEFTDOWN) || flags.contains(MOUSEEVENTF_LEFTUP) {
                        (1, flags.contains(MOUSEEVENTF_LEFTDOWN), MOUSEEVENTF_LEFTUP)
                    } else if flags.contains(MOUSEEVENTF_RIGHTDOWN)
                        || flags.contains(MOUSEEVENTF_RIGHTUP)
                    {
                        (
                            2,
                            flags.contains(MOUSEEVENTF_RIGHTDOWN),
                            MOUSEEVENTF_RIGHTUP,
                        )
                    } else {
                        continue;
                    };
                let mut release = *event;
                release.Anonymous.mi.dwFlags = up;
                ((code, 0, 0), down, release)
            } else {
                continue;
            }
        };
        pressed.retain(|(held, _)| *held != key);
        if down {
            pressed.push((key, release));
        }
    }
    pressed
        .into_iter()
        .rev()
        .map(|(_, release)| release)
        .collect()
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/input/submission.rs"]
mod tests;
