//! 真实网页的只读列表采集；用连续快照确认条目稳定，不降级到本地 HTML。
use super::Result;
use argusflow_browser::Page;
use argusflow_core::OperationOptions;
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HotItem {
    pub rank: u32,
    pub title: String,
    pub url: String,
    pub selector: String,
    pub row_class: String,
    pub title_class: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct HotList {
    pub url: String,
    pub captured_at: String,
    pub root_selector: String,
    pub list_selector: String,
    pub root_class: String,
    pub items: Vec<HotItem>,
}
pub async fn discover(page: &Page, output: &Path) -> Result<HotList> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut previous = None;
    loop {
        let value = page
            .evaluate(include_str!("baidu-list.js"), OperationOptions::default())
            .await?;
        if !value.is_null() {
            let list: HotList = serde_json::from_value(value)?;
            if previous.as_ref() == Some(&list.items) {
                std::fs::write(
                    output.join("live-hotsearch.json"),
                    serde_json::to_vec_pretty(&list)?,
                )?;
                return Ok(list);
            }
            previous = Some(list.items);
        } else {
            previous = None;
        }
        if Instant::now() >= deadline {
            return Err("真实百度热搜列表未稳定就绪，停止回放".into());
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}
