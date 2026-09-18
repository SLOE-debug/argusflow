//! 使用同一 Rust 谓词语义筛选按阅读顺序排列的 OCR 文本。
use crate::OcrResult;
use argusflow_aql::{Attribute, BoundQuery, Capabilities, Node, QueryTree, Role, Value, evaluate};
use argusflow_core::{Failure, ImagePoint, Operation};
use std::collections::BTreeMap;

/// 识别文字的独立快照，不表示它是可点击控件。
#[derive(Debug, Clone)]
pub struct OcrMatch {
    index: usize,
    text: String,
    confidence: f32,
    polygon: [ImagePoint; 4],
}
impl OcrMatch {
    /// 在本次识别结果中的阅读顺序索引。
    pub fn index(&self) -> usize {
        self.index
    }
    /// 完整识别文字。
    pub fn text(&self) -> &str {
        &self.text
    }
    /// 平均保留字符置信度。
    pub fn confidence(&self) -> f32 {
        self.confidence
    }
    /// 输入图像坐标中的四边形。
    pub fn polygon(&self) -> &[ImagePoint; 4] {
        &self.polygon
    }
}
/// OCR 不推断控件角色、层级或可交互状态。
pub fn ocr_query_capabilities() -> Capabilities {
    Capabilities::new([Role::Text], [Attribute::Text, Attribute::Confidence])
}
impl OcrResult {
    /// 在已识别图片中定位文字；不创建屏幕身份，也不执行点击。
    pub fn query_aql(
        &self,
        query: &BoundQuery,
        operation: &Operation,
    ) -> Result<Vec<OcrMatch>, Failure> {
        ocr_query_capabilities().check(query)?;
        let tree = self.aql_tree(operation)?;
        let indices = evaluate(query, &tree, operation)?;
        Ok(indices
            .into_iter()
            .map(|index| {
                let block = &self.blocks()[index];
                OcrMatch {
                    index,
                    text: block.text().into(),
                    confidence: block.confidence(),
                    polygon: *block.polygon(),
                }
            })
            .collect())
    }
    /// 使用同一识别快照生成候选排名与排除理由，不执行输入。
    pub fn preview_aql(
        &self,
        query: &BoundQuery,
        operation: &Operation,
    ) -> Result<Vec<argusflow_aql::SpatialPreview>, Failure> {
        ocr_query_capabilities().check(query)?;
        argusflow_aql::preview(query, &self.aql_tree(operation)?, operation)
    }
    fn aql_tree(&self, operation: &Operation) -> Result<QueryTree<usize>, Failure> {
        let mut tree = QueryTree::new(3000, 1, 256)?;
        let scope = argusflow_aql::Rect::new([
            0.0,
            0.0,
            f64::from(self.width()),
            f64::from(self.height()),
        ])?;
        for (index, block) in self.blocks().iter().enumerate() {
            operation.check("ocr_aql_snapshot")?;
            let values = BTreeMap::from([
                (Attribute::Text, Value::Text(block.text().into())),
                (
                    Attribute::Confidence,
                    Value::Number(f64::from(block.confidence())),
                ),
            ]);
            let left = block
                .polygon()
                .iter()
                .map(|p| f64::from(p.x))
                .fold(f64::INFINITY, f64::min);
            let top = block
                .polygon()
                .iter()
                .map(|p| f64::from(p.y))
                .fold(f64::INFINITY, f64::min);
            let right = block
                .polygon()
                .iter()
                .map(|p| f64::from(p.x))
                .fold(f64::NEG_INFINITY, f64::max);
            let bottom = block
                .polygon()
                .iter()
                .map(|p| f64::from(p.y))
                .fold(f64::NEG_INFINITY, f64::max);
            let mut node = Node::element(None, Role::Text, values, index);
            if let Ok(bounds) = argusflow_aql::Rect::new([left, top, right - left, bottom - top]) {
                node = node.with_geometry(argusflow_aql::Geometry::new(
                    bounds,
                    scope,
                    "ocr_image",
                    None,
                )?);
            }
            tree.push(node)?;
        }
        Ok(tree)
    }
}
#[cfg(test)]
#[path = "../../../../tests/argusflow-vision/unit/aql/query.rs"]
mod tests;
