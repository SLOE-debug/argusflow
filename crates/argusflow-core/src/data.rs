use serde::{Deserialize, Serialize};

use crate::ValueExpr;

/// 把结构化对象数组格式化为确定文本的语义数据操作。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelimitedTextFormat {
    /// 解析后必须是对象数组的输入值。
    pub items: ValueExpr,
    /// 按输出顺序读取的非空字段名。
    pub fields: Vec<String>,
    /// 字段之间插入的分隔文本。
    pub column_separator: String,
    /// 每条记录之后插入的行分隔文本。
    pub row_separator: String,
    /// 是否在首行输出字段名称。
    pub include_header: bool,
}
