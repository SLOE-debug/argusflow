//! 显式运行的示例只创建自己的临时页面。
use argusflow_aql::{Bindings, Value, compile};
use argusflow_automation::{Locator, QuerySource};
use argusflow_browser::{Browser, BrowserConfig};
use argusflow_core::OperationOptions;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::args().nth(1).ok_or("需要显式 CDP 地址")?;
    let browser = Browser::connect(
        &endpoint,
        BrowserConfig::default(),
        OperationOptions::default(),
    )
    .await?;
    let page = browser
        .new_page("about:blank", OperationOptions::default())
        .await?;
    page.navigate(
        "data:text/html,<input aria-label='Message' value='existing'><button>Save</button>",
        OperationOptions::default(),
    )
    .await?;
    let query = compile("textbox(name = $label)")?;
    let locator = Locator::bind(
        QuerySource::Browser(page.clone()),
        &query,
        &Bindings::from([("label".into(), Value::Text("Message".into()))]),
    )?;
    locator
        .type_text("追加内容", OperationOptions::default())
        .await?;
    page.close(OperationOptions::default()).await?;
    browser.shutdown(OperationOptions::default()).await?;
    Ok(())
}
