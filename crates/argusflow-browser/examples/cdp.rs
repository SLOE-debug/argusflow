//! 用户显式提供 Browser WebSocket / HTTP 端点后手动运行。
use argusflow_browser::{Browser, BrowserConfig, OperationOptions};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::args()
        .nth(1)
        .ok_or("usage: cdp <explicit debugging endpoint>")?;
    let browser = Browser::connect(
        &endpoint,
        BrowserConfig::default(),
        OperationOptions::default(),
    )
    .await?;
    let result = async {
        for page in browser.pages(OperationOptions::default()).await? {
            println!("{} {}", page.target_id(), page.title());
        }
        let page = browser
            .new_page("about:blank", OperationOptions::default())
            .await?;
        page.evaluate(
            "document.body.innerHTML='<button id=demo>Example</button>'",
            OperationOptions::default(),
        )
        .await?;
        let element = page
            .find_unique("#demo", OperationOptions::default())
            .await?;
        println!("{}", element.read(OperationOptions::default()).await?.text);
        page.close(OperationOptions::default()).await?;
        Ok::<_, argusflow_browser::BrowserError>(())
    }
    .await;
    browser.shutdown(OperationOptions::default()).await?;
    result?;
    Ok(())
}
