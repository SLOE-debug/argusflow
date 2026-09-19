//! 浏览器返回数据的类型和 UTF-8 文本输出。
use serde::Deserialize;
use std::{
    error::Error,
    fmt::Write,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
pub struct Item {
    /// 热搜标题。
    pub title: String,
    /// 百度热搜条目链接。
    pub url: String,
}

#[derive(Deserialize)]
pub struct Report {
    pub page_title: String,
    pub page_url: String,
    /// Node 生成的 ISO 8601 采集时间。
    pub captured_at: String,
    pub items: Vec<Item>,
}

/// 仅在非空结果成功返回后写入文件，失败不覆盖已有结果。
pub fn save(directory: &Path, report: &Report) -> Result<PathBuf, Box<dyn Error>> {
    if report.items.is_empty() {
        return Err("百度热搜列表为空，不保存成功结果".into());
    }
    let mut text = format!(
        "百度热搜列表\n页面：{}\n来源：{}\n采集时间：{}\n\n",
        report.page_title, report.page_url, report.captured_at
    );
    for (index, item) in report.items.iter().enumerate() {
        writeln!(text, "{}. {}\n   {}", index + 1, item.title, item.url)?;
    }
    let output = directory.join("output/baidu-hot-list.txt");
    std::fs::create_dir_all(directory.join("output"))?;
    std::fs::write(&output, text)?;
    Ok(output)
}
