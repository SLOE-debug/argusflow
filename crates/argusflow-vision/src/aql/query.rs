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
    Capabilities::new(
        [Role::Text],
        [Attribute::Name, Attribute::Text, Attribute::Confidence],
    )
}
impl OcrResult {
    /// 在已识别图片中定位文字；不创建屏幕身份，也不执行点击。
    pub fn query_aql(
        &self,
        query: &BoundQuery,
        operation: &Operation,
    ) -> Result<Vec<OcrMatch>, Failure> {
        ocr_query_capabilities().check(query)?;
        let mut tree = QueryTree::new(3000, 1, 256)?;
        for (index, block) in self.blocks().iter().enumerate() {
            operation.check("ocr_aql_snapshot")?;
            let values = BTreeMap::from([
                (Attribute::Name, Value::Text(block.text().into())),
                (Attribute::Text, Value::Text(block.text().into())),
                (
                    Attribute::Confidence,
                    Value::Number(f64::from(block.confidence())),
                ),
            ]);
            tree.push(Node::element(None, Role::Text, values, index))?;
        }
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
}
#[cfg(test)]
#[path = "../../../../tests/argusflow-vision/unit/aql/query.rs"]
mod tests;
