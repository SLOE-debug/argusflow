//! 视觉节点的行、row 和 region 几何聚合。

mod lines;
mod rows;

pub use lines::{VisualLine, VisualLineId, cluster_lines};
pub use rows::{RowConfig, VisualRow, VisualRowId, cluster_rows};
