//! 真实 Chromium 验收；只启动独立配置目录和本地合成页面。
use argusflow_browser::{Browser, BrowserConfig, LaunchOptions, OperationOptions};
use argusflow_core::{ClickCount, CssPoint, FailureKind, Key, MouseButton, ScrollAxis};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};

fn executable() -> PathBuf {
    std::env::var_os("ARGUSFLOW_TEST_BROWSER")
        .expect("set ARGUSFLOW_TEST_BROWSER to Chrome/Edge executable")
        .into()
}
fn options() -> OperationOptions {
    OperationOptions::default()
}

const CONTENT: &str = r#"<html><body style="margin:20px;width:2000px;height:3000px">
<button id="button" onclick="window.clicks++" oncontextmenu="window.rights++;event.preventDefault()" ondblclick="window.doubles++">Click target</button>
<button class="duplicate">A</button><button class="duplicate">B</button>
<input id="input" aria-label="Input test"><div id="shadow"></div><iframe srcdoc="<button id='in-frame'>inside</button>"></iframe>
<button id="bottom" style="position:absolute;top:2400px">Bottom target</button>
</body></html>"#;

#[tokio::test]
#[ignore = "launches an isolated real browser; explicitly set ARGUSFLOW_TEST_BROWSER"]
async fn browser_dom_input_navigation_and_shutdown() {
    let mut launch = LaunchOptions::new(executable());
    launch.headless = true;
    let browser = Browser::launch(launch, BrowserConfig::default())
        .await
        .expect("launch browser");
    let result = exercise(&browser).await;
    let shutdown = browser.shutdown(options()).await;
    result.expect("browser acceptance");
    shutdown.expect("browser shutdown");
    assert_eq!(
        browser.state(),
        argusflow_browser::ConnectionState::Disconnected
    );
}

async fn exercise(browser: &Browser) -> Result<(), Box<dyn std::error::Error>> {
    assert!(!browser.pages(options()).await?.is_empty());
    println!("PASS launch and page listing");
    let page = browser.new_page("about:blank", options()).await?;
    page.evaluate(&format!("document.open();document.write({});document.close();window.clicks=0;window.rights=0;window.doubles=0;document.querySelector('#shadow').attachShadow({{mode:'open'}}).innerHTML='<button id=inside-shadow>shadow</button>';",json!(CONTENT)),options()).await?;
    let button = page.find_unique("#button", options()).await?;
    let snapshot = button.read(options()).await?;
    assert_eq!(snapshot.text, "Click target");
    assert_eq!(
        snapshot.attributes.get("id").map(String::as_str),
        Some("button")
    );
    assert_eq!(
        page.find_unique(".duplicate", options())
            .await
            .err()
            .unwrap()
            .kind(),
        FailureKind::Ambiguous
    );
    assert_eq!(page.find_all("#inside-shadow", options()).await?.len(), 0);
    assert_eq!(page.find_all("#in-frame", options()).await?.len(), 0);
    println!("PASS CSS, attributes, ambiguity and main-document scope");
    button
        .click(MouseButton::Left, ClickCount::Single, options())
        .await?;
    assert_eq!(page.evaluate("clicks", options()).await?, 1);
    button
        .click(MouseButton::Right, ClickCount::Single, options())
        .await?;
    assert_eq!(page.evaluate("rights", options()).await?, 1);
    button
        .click(MouseButton::Left, ClickCount::Double, options())
        .await?;
    assert_eq!(page.evaluate("doubles", options()).await?, 1);
    assert_eq!(page.evaluate("clicks", options()).await?, 3);
    println!("PASS left/right/double click");
    let input = page.find_unique("#input", options()).await?;
    input.focus(options()).await?;
    page.insert_text("中文 input 123", options()).await?;
    assert_eq!(
        page.evaluate("document.querySelector('#input').value", options())
            .await?,
        "中文 input 123"
    );
    page.press(&[Key::Control, Key::Letter('a')], options())
        .await?;
    page.insert_text("replaced 456", options()).await?;
    assert_eq!(
        page.evaluate("document.querySelector('#input').value", options())
            .await?,
        "replaced 456"
    );
    println!("PASS focus, Unicode text and Ctrl+A");
    let bottom = page.find_unique("#bottom", options()).await?;
    bottom.scroll_into_view(options()).await?;
    assert!(page.evaluate("scrollY", options()).await?.as_f64().unwrap() > 1000.0);
    page.evaluate("scrollTo(0,0)", options()).await?;
    page.wheel(
        CssPoint::new(400.0, 200.0)?,
        ScrollAxis::Vertical,
        300.0,
        options(),
    )
    .await?;
    wait_expression(&page, "scrollY>0").await?;
    page.wheel(
        CssPoint::new(400.0, 200.0)?,
        ScrollAxis::Horizontal,
        200.0,
        options(),
    )
    .await?;
    wait_expression(&page, "scrollX>0").await?;
    println!("PASS scroll into view and both wheel axes");
    page.navigate(
        "data:text/html,<title>new</title><p id='new'>replacement</p>",
        options(),
    )
    .await?;
    assert_eq!(
        button.read(options()).await.unwrap_err().kind(),
        FailureKind::StaleHandle
    );
    assert_eq!(
        page.find_unique("#new", options())
            .await?
            .read(options())
            .await?
            .text,
        "replacement"
    );
    let current = page.find_unique("#new", options()).await?;
    page.close(options()).await?;
    assert_eq!(
        current.read(options()).await.unwrap_err().kind(),
        FailureKind::StaleHandle
    );
    println!("PASS navigation, invalidation and close");
    Ok(())
}

async fn wait_expression(
    page: &argusflow_browser::Page,
    expression: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if page.evaluate(expression, options()).await? == Value::Bool(true) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("condition not observed: {expression}").into());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
