//! 示范者按实际搜索结果打开明确授权的接收人；不向推导器传递接收人。
use super::{
    Result,
    wechat_support::{
        conversation, desktop::Desktop, layout::Zone, observation::Observer, snapshot::Observation,
    },
};
use argusflow_core::{Key, OperationOptions};
use argusflow_windows::InputAction;
use std::time::Duration;
pub async fn open(
    desktop: &Desktop,
    observer: &mut Observer,
    recipient: &str,
) -> Result<Observation> {
    let mut initial = observer.refresh(desktop).await?;
    let header = Zone::Header.region(initial.frame.bounds)?;
    let titles = initial.text(header, recipient, false)?;
    if titles.iter().any(|t| {
        t.point.x > initial.frame.bounds.x() + initial.frame.bounds.width() as i32 * 35 / 100
    }) {
        return Ok(initial);
    }
    if initial.text(header, "搜索", true)?.is_empty() {
        desktop.key(Key::Escape).await?;
        tokio::time::sleep(Duration::from_millis(250)).await;
        initial = observer.refresh(desktop).await?;
    }
    let sidebar = Zone::Sidebar.region(initial.frame.bounds)?;
    let first = initial
        .blocks
        .iter()
        .filter(|b| sidebar.contains(b.rect) && b.confidence >= 0.65)
        .min_by_key(|b| b.rect.y())
        .ok_or("左侧列表没有可定位项目")?;
    let point = initial.frame.bounds.project(first.rect)?;
    desktop
        .click(
            argusflow_core::ScreenPoint {
                x: point.x() + point.width() as i32 / 2,
                y: point.y() + point.height() as i32 / 2,
            },
            initial.frame.bounds,
        )
        .await?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    desktop.key(Key::Home).await?;
    for page in 0..=3 {
        tokio::time::sleep(Duration::from_millis(600)).await;
        let current = observer.refresh(desktop).await?;
        let matches = current.text(
            Zone::Sidebar.region(current.frame.bounds)?,
            recipient,
            false,
        )?;
        if let [contact] = matches.as_slice() {
            desktop.click(contact.point, current.frame.bounds).await?;
            tokio::time::sleep(Duration::from_millis(800)).await;
            return conversation::ensure_open(desktop, observer).await;
        }
        if matches.len() > 1 {
            return Err("列表中出现同名联系人，停止选择".into());
        }
        if page < 3 {
            println!("列表第{}次 PageDown", page + 1);
            desktop.key(Key::PageDown).await?;
        }
    }
    println!("顶部及三次翻页均未找到，使用 Ctrl+F 搜索");
    desktop
        .input
        .perform(
            desktop.window.clone(),
            InputAction::Chord(vec![Key::Control, Key::Letter('F')]),
            OperationOptions::default(),
        )
        .await?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    desktop
        .input
        .perform(
            desktop.window.clone(),
            InputAction::Chord(vec![Key::Control, Key::Letter('A')]),
            OperationOptions::default(),
        )
        .await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    desktop.type_text(recipient).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let screen = super::screen::Screen::start()?;
    let frame = screen.frame(desktop).await;
    screen.shutdown().await?;
    let result = observer.recognize_frame(desktop, frame?).await?;
    let matches = result.text(Zone::Sidebar.region(result.frame.bounds)?, recipient, false)?;
    let [contact] = matches.as_slice() else {
        return Err("搜索结果不唯一，停止打开联系人".into());
    };
    desktop.click(contact.point, result.frame.bounds).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    conversation::ensure_open(desktop, observer).await
}

pub async fn check(root: &std::path::Path) -> Result<()> {
    let window = argusflow_windows::WindowLocator {
        title: Some("微信".into()),
        ..Default::default()
    }
    .find_unique()?
    .identity();
    let runtime =
        argusflow_windows::UiaRuntime::start(Default::default(), OperationOptions::default())
            .await?;
    let input = argusflow_windows::InputService::new()?;
    window.activate(&argusflow_core::Operation::new(OperationOptions::default()))?;
    let mut config = argusflow_vision::OcrConfig::new(root.join(".deps"));
    config.detection_min_side = 64;
    let engine = argusflow_vision::OcrEngine::load(config).await?;
    let desktop = Desktop { window, input };
    let result = open(&desktop, &mut Observer::new(engine.clone()), "文件传输助手")
        .await
        .map(|_| ());
    engine.shutdown(OperationOptions::default()).await?;
    desktop.input.shutdown(OperationOptions::default()).await?;
    runtime.shutdown(OperationOptions::default()).await?;
    result
}
