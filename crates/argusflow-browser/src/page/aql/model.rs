//! CDP JSON 只在适配层解码，AQL 不读取协议对象。
use super::context::DocumentContext;
use argusflow_aql::{Attribute, Role, Value};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DomNode {
    pub node_id: i64,
    pub backend_node_id: i64,
    pub node_type: u8,
    #[serde(default)]
    pub local_name: String,
    #[serde(default)]
    pub children: Vec<Arc<DomNode>>,
    #[serde(default)]
    pub shadow_roots: Vec<Arc<DomNode>>,
    pub shadow_root_type: Option<String>,
    pub content_document: Option<Arc<DomNode>>,
    pub frame_id: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct DomResponse {
    pub root: Option<Arc<DomNode>>,
    pub node: Option<Arc<DomNode>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AxNode {
    #[serde(rename = "backendDOMNodeId")]
    pub backend_dom_node_id: Option<i64>,
    pub role: Option<AxValue>,
    pub name: Option<AxValue>,
}
#[derive(Deserialize)]
pub(super) struct AxValue {
    pub value: serde_json::Value,
}
#[derive(Deserialize)]
pub(super) struct AxResponse {
    pub nodes: Vec<AxNode>,
}
#[derive(Deserialize)]
pub(super) struct DomFacts {
    pub path: String,
    pub text: Option<String>,
    pub value: Option<String>,
    pub enabled: Option<bool>,
    pub visible: bool,
    pub focused: bool,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub attributes: BTreeMap<String, String>,
    pub css: Vec<String>,
    pub bounds: [f64; 4],
}
/// 绑定文档代际的浏览器定位结果；不长期保留 JS RemoteObject。
#[derive(Clone)]
pub struct BrowserMatch {
    pub(super) context: Arc<DocumentContext>,
    pub(super) node: Arc<DomNode>,
    pub(super) role: Role,
    pub(super) attributes: BTreeMap<Attribute, Value>,
    pub(super) bounds: [f64; 4],
}
impl BrowserMatch {
    /// 可访问角色或通用元素角色。
    pub fn role(&self) -> Role {
        self.role
    }
    /// 查询时的已知属性，缺失值不会填充。
    pub fn attributes(&self) -> &BTreeMap<Attribute, Value> {
        &self.attributes
    }
    /// 所属 frame 视口内的 CSS 像素矩形。
    pub fn bounds(&self) -> [f64; 4] {
        self.bounds
    }
    /// 定位结果所属 frame 标识。
    pub fn frame_id(&self) -> &str {
        &self.context.frame_id
    }
}
