//! 全窗口文本快照与 AQL 查询；区域只限制查询，不触发新的 OCR。
use super::{frame::Frame, spatial::Candidate, text::TextRecord};
use argusflow_aql::{Attribute, Bindings, Node, QueryTree, Role, Value, compile, evaluate};
use argusflow_capture_contracts::PixelRect;
use argusflow_core::{Operation, OperationOptions, ScreenPoint};
use std::{collections::BTreeMap, error::Error, sync::Arc};

/// 全窗口文字基线；保留像素用于点击前校验。
#[derive(Clone)]
pub struct Observation {
    /// 本轮完整窗口像素，屏幕原点改变时自动重新投影。
    pub frame: Frame,
    /// 窗口局部坐标中的合并文字，不包含已消失块。
    pub blocks: Arc<Vec<TextRecord>>,
}
impl Observation {
    /// 原图所有文字与屏幕坐标；仅由显式 observe 模式输出。
    pub fn print(&self) {
        for block in self.blocks.iter() {
            println!(
                "confidence={:.3} rect={:?} text={}",
                block.confidence,
                self.frame.bounds.project(block.rect),
                block.text
            );
        }
    }
    /// 在同一快照中按 AQL 精确匹配或包含匹配查询。
    pub fn text(
        &self,
        region: PixelRect,
        text: &str,
        contains: bool,
    ) -> Result<Vec<Candidate>, Box<dyn Error>> {
        let expression = if contains {
            "text(text contains $label, confidence >= 0.65)"
        } else {
            "text(text = $label, confidence >= 0.65)"
        };
        let query = compile(expression)?.bind(&Bindings::from([(
            "label".into(),
            Value::Text(text.into()),
        )]))?;
        let mut tree = QueryTree::new(10000, 1, 4096)?;
        for (index, block) in self
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| region.contains(b.rect))
        {
            tree.push(Node::element(
                None,
                Role::Text,
                BTreeMap::from([
                    (Attribute::Text, Value::Text(block.text.clone())),
                    (
                        Attribute::Confidence,
                        Value::Number(f64::from(block.confidence)),
                    ),
                ]),
                index,
            ))?;
        }
        let operation = Operation::new(OperationOptions::default());
        evaluate(&query, &tree, &operation)?
            .into_iter()
            .map(|node_index| {
                let index = *tree
                    .node(node_index)
                    .and_then(|n| n.target())
                    .ok_or("查询索引失效")?;
                let block = &self.blocks[index];
                Ok(Candidate {
                    text: block.text.clone(),
                    index,
                    point: ScreenPoint {
                        x: self.frame.bounds.x()
                            + (block.polygon.iter().map(|p| p.x).sum::<f32>() / 4.0).round() as i32,
                        y: self.frame.bounds.y()
                            + (block.polygon.iter().map(|p| p.y).sum::<f32>() / 4.0).round() as i32,
                    },
                })
            })
            .collect()
    }
    /// 是否在指定区域识别到任何文字，不能代替可编辑性判定。
    pub fn is_empty(&self, region: PixelRect) -> bool {
        !self
            .blocks
            .iter()
            .any(|b| region.intersection(b.rect).is_some())
    }
}
