//! OCR 只承诺文字与几何，不能伪造 UI 控件语义。
use super::*;
use crate::TextBlock;
use argusflow_aql::{Bindings, compile};
use argusflow_core::OperationOptions;
#[test]
fn ocr_query_filters_confidence_and_keeps_reading_order() {
    let result = OcrResult {
        width: 100,
        height: 100,
        blocks: vec![
            TextBlock {
                text: "保存".into(),
                confidence: 0.95,
                polygon: [ImagePoint { x: 1.0, y: 2.0 }; 4],
            },
            TextBlock {
                text: "保存".into(),
                confidence: 0.5,
                polygon: [ImagePoint { x: 1.0, y: 3.0 }; 4],
            },
        ],
    };
    let operation = Operation::new(OperationOptions::default());
    let query = compile("text(name = \"保存\", confidence > 0.9)")
        .unwrap()
        .bind(&Bindings::new())
        .unwrap();
    let found = result.query_aql(&query, &operation).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].index(), 0);
    let unsupported = compile("button(name = \"保存\")")
        .unwrap()
        .bind(&Bindings::new())
        .unwrap();
    assert_eq!(
        result
            .query_aql(&unsupported, &operation)
            .unwrap_err()
            .kind(),
        argusflow_core::FailureKind::Unsupported
    );
}
