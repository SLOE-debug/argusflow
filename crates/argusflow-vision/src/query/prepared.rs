//! 旧视觉文本查询与 AQL Vision 计划的统一准备态表示。

use std::collections::BTreeMap;

use argusflow_core::{AqlQuery, VisualQuery};
use argusflow_query::{parse_stored_query, resolve_query_parameters};

use super::{VisionQueryPlan, compile_vision_query};

/// Vision backend 执行前已经冻结的查询形态。
#[derive(Debug, Clone, PartialEq)]
pub enum PreparedVisionQuery {
    /// 迁移期间保留的 VisualQueryExpr 解析结果。
    Legacy(VisualQuery),
    /// AQL 编译产生的结构化场景查询计划。
    Aql {
        /// 用户可读源码，仅用于错误与 Evidence。
        source: String,
        /// 不再解释通用 AQL 的执行计划。
        plan: VisionQueryPlan,
    },
}

impl PreparedVisionQuery {
    /// 解析绑定并编译持久化 AQL，不进行源码字符串插值。
    pub fn from_aql(
        query: &AqlQuery,
        parameters: &BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let parsed = parse_stored_query(query).map_err(|error| error.to_string())?;
        let resolved =
            resolve_query_parameters(&parsed, parameters).map_err(|error| error.to_string())?;
        let plan = compile_vision_query(&resolved).map_err(|error| error.to_string())?;
        Ok(Self::Aql {
            source: query.source.clone(),
            plan,
        })
    }

    /// 返回 Explain 与统一错误使用的查询文本。
    pub fn source(&self) -> &str {
        match self {
            Self::Legacy(query) => &query.text,
            Self::Aql { source, .. } => source,
        }
    }

    /// 返回本计划的稳定 Explain 步骤。
    pub fn summary(&self) -> Vec<String> {
        match self {
            Self::Legacy(query) => vec![if query.exact {
                "legacy exact visual text query".to_owned()
            } else {
                "legacy contains visual text query".to_owned()
            }],
            Self::Aql { plan, .. } => plan.summary.clone(),
        }
    }
}
