//! OCR Scene 的 AQL 编译与确定性求值入口。

mod aql;

pub(crate) use aql::evaluate_window_query;
pub use aql::{
    VisionQueryCompileError, VisionQueryMetrics, VisionQueryPlan, VisionQueryResult,
    compile_vision_query, evaluate_vision_query, require_unique,
};
