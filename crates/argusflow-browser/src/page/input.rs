//! 页面输入和取消时释放本次按键的守卫。
use crate::BrowserError as Failure;
use crate::{Page, cdp::Cleanup};
use argusflow_core::{ClickCount, CssPoint, FailureKind, Key, MouseButton, Operation, ScrollAxis};
use serde_json::json;
use std::collections::HashSet;
use tokio::sync::OwnedSemaphorePermit;

pub(crate) fn reserve(page: &Page) -> Result<OwnedSemaphorePermit, Failure> {
    page.inner
        .connection
        .inner
        .input
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            Failure::new(
                FailureKind::Busy,
                "cdp_input",
                "连接上已有输入或按键清理正在执行",
            )
        })
}
fn cleanup(page: &Page, permit: OwnedSemaphorePermit) -> Cleanup {
    Cleanup::new(
        page.inner.connection.clone(),
        Some(page.inner.session.clone()),
        permit,
    )
}

pub(crate) async fn text(page: &Page, text: &str, operation: &Operation) -> Result<(), Failure> {
    if text.is_empty() || text.len() > 16_384 {
        return Err(invalid("输入文本为空或超过 16 KiB"));
    }
    let _permit = reserve(page)?;
    page.command("Input.insertText", json!({"text":text}), true, operation)
        .await?;
    Ok(())
}
pub(crate) async fn wheel(
    page: &Page,
    point: CssPoint,
    axis: ScrollAxis,
    delta: f64,
    operation: &Operation,
) -> Result<(), Failure> {
    if !delta.is_finite() || delta == 0.0 || delta.abs() > 1_000_000.0 {
        return Err(invalid("滚轮位移必须为有限非零值且不超过一百万 CSS 像素"));
    }
    let _permit = reserve(page)?;
    let (x, y) = match axis {
        ScrollAxis::Horizontal => (delta, 0.0),
        ScrollAxis::Vertical => (0.0, delta),
    };
    page.command(
        "Input.dispatchMouseEvent",
        json!({"type":"mouseWheel","x":point.x(),"y":point.y(),"deltaX":x,"deltaY":y}),
        true,
        operation,
    )
    .await?;
    Ok(())
}
pub(crate) async fn click(
    page: &Page,
    point: CssPoint,
    button: MouseButton,
    count: ClickCount,
    operation: &Operation,
    permit: OwnedSemaphorePermit,
) -> Result<(), Failure> {
    let button = match button {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
    };
    page.command(
        "Input.dispatchMouseEvent",
        json!({"type":"mouseMoved","x":point.x(),"y":point.y()}),
        true,
        operation,
    )
    .await?;
    let mut cleanup = cleanup(page, permit);
    for index in 1..=if count == ClickCount::Double { 2 } else { 1 } {
        let release = json!({"type":"mouseReleased","x":point.x(),"y":point.y(),"button":button,"clickCount":index});
        // 发布释放守卫必须先于发送按下；按下响应丢失也需要尝试释放。
        cleanup
            .commands
            .push(("Input.dispatchMouseEvent", release.clone()));
        page.command("Input.dispatchMouseEvent",json!({"type":"mousePressed","x":point.x(),"y":point.y(),"button":button,"clickCount":index}),true,operation).await?;
        page.command("Input.dispatchMouseEvent", release, true, operation)
            .await?;
        cleanup.commands.pop();
    }
    Ok(())
}
pub(crate) async fn keys(page: &Page, keys: &[Key], operation: &Operation) -> Result<(), Failure> {
    if keys.is_empty() || keys.len() > 8 {
        return Err(invalid("组合键数量必须在 1-8 之间"));
    }
    let mut seen = HashSet::new();
    let definitions = keys
        .iter()
        .map(|key| key_definition(*key))
        .collect::<Result<Vec<_>, _>>()?;
    for definition in &definitions {
        if !seen.insert(definition.1) {
            return Err(invalid("组合键包含重复按键"));
        }
    }
    let mut cleanup = cleanup(page, reserve(page)?);
    let mut modifiers = 0;
    for (name, virtual_key, modifier) in definitions {
        modifiers |= modifier;
        cleanup.commands.push(("Input.dispatchKeyEvent",json!({"type":"keyUp","key":name,"windowsVirtualKeyCode":virtual_key,"modifiers":modifiers&!modifier})));
        page.command("Input.dispatchKeyEvent",json!({"type":"rawKeyDown","key":name,"windowsVirtualKeyCode":virtual_key,"modifiers":modifiers}),true,operation).await?;
    }
    while let Some((method, release)) = cleanup.commands.last().cloned() {
        page.command(method, release, true, operation).await?;
        cleanup.commands.pop();
    }
    Ok(())
}

fn key_definition(key: Key) -> Result<(String, u16, u8), Failure> {
    let (name, code, modifier) = match key {
        Key::Control => ("Control", 17, 2),
        Key::Shift => ("Shift", 16, 8),
        Key::Alt => ("Alt", 18, 1),
        Key::Meta => ("Meta", 91, 4),
        Key::Enter => ("Enter", 13, 0),
        Key::Tab => ("Tab", 9, 0),
        Key::Escape => ("Escape", 27, 0),
        Key::Backspace => ("Backspace", 8, 0),
        Key::Delete => ("Delete", 46, 0),
        Key::Space => (" ", 32, 0),
        Key::Home => ("Home", 36, 0),
        Key::End => ("End", 35, 0),
        Key::PageUp => ("PageUp", 33, 0),
        Key::PageDown => ("PageDown", 34, 0),
        Key::Left => ("ArrowLeft", 37, 0),
        Key::Up => ("ArrowUp", 38, 0),
        Key::Right => ("ArrowRight", 39, 0),
        Key::Down => ("ArrowDown", 40, 0),
        Key::Letter(letter) if letter.is_ascii_alphabetic() => {
            return Ok((
                letter.to_ascii_lowercase().to_string(),
                letter.to_ascii_uppercase() as u16,
                0,
            ));
        }
        Key::Function(number) if (1..=12).contains(&number) => {
            return Ok((format!("F{number}"), 111 + u16::from(number), 0));
        }
        Key::Letter(_) | Key::Function(_) => return Err(invalid("不支持此字母键或功能键")),
    };
    Ok((name.into(), code, modifier))
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailureKind::InvalidInput, "cdp_input", message)
}
