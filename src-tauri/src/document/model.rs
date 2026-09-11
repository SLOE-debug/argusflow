//! 编辑文件和执行定义在传输边界分离。
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 可保存未完成配置的当前编辑文件。
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowFile {
    /// 当前编辑文件版本。
    pub format_version: u32,
    /// 与文件路径无关的稳定身份。
    pub id: String,
    /// 带十进制整数字符串的前端执行定义。
    pub definition: serde_json::Value,
    /// 非执行布局和无效字段草稿。
    pub editor: EditorData,
}
/// 画布布局及字段编辑原文。
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorData {
    /// 节点对应的布局。
    pub nodes: BTreeMap<String, NodeLayout>,
    /// 尚未编译的字段原文，键为节点 ID 和字段名。
    pub drafts: BTreeMap<String, String>,
}
/// 单个节点的局部位置和展示文字。
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeLayout {
    /// 作用域内横坐标。
    pub x: f64,
    /// 作用域内纵坐标。
    pub y: f64,
    /// 用户可编辑标题。
    pub label: String,
    /// 非执行备注。
    pub note: String,
}
/// 打开文件的内容和乐观并发版本。
#[derive(Clone, Serialize)]
pub struct LoadedDocument {
    /// 编辑文件。
    pub file: WorkflowFile,
    /// 文件内容哈希。
    pub revision: String,
}
/// 工作目录列表条目。
#[derive(Clone, Serialize)]
pub struct DocumentSummary {
    /// 稳定文档身份。
    pub id: String,
    /// 显示名称。
    pub name: String,
    /// 内容哈希。
    pub revision: String,
}
