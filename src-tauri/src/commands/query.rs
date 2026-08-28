//! AQL 编辑器使用的语言检查与真实 Runtime Planner Explain 命令。

use argusflow_agent::PlanningReport;
use argusflow_core::{AutomationAction, AutomationTarget, TargetLocator};
use argusflow_query::{Diagnostic, QueryPortability, analyze_query, parse_document};
use serde::Serialize;
use tauri::State;

use crate::runtime::AppState;

/// AQL Planner 检查结果；语法高亮等即时反馈由同一 Rust crate 的 WASM 接口提供。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AqlInspection {
    /// 查询已完成 HIR lowering，并附带真实 backend prepared candidate 报告。
    Valid {
        /// 适合 cache key 与 identity 的 canonical AQL。
        canonical_source: String,
        /// 查询是否依赖 backend namespace。
        portability: QueryPortability,
        /// 与 backend 能力无关的语义诊断。
        diagnostics: Vec<Diagnostic>,
        /// 由当前 ExecutionContext 和真实 compiler plan 生成的候选报告。
        planning: PlanningReport,
    },
    /// 查询不完整或无效；恢复 parser 可以一次返回多项诊断。
    Invalid {
        /// 浏览器安全的 UTF-16 诊断列表。
        diagnostics: Vec<Diagnostic>,
    },
}

#[tauri::command]
/// 检查持久化 AQL，并让 Runtime Planner 基于当前上下文准备真实候选。
pub fn inspect_aql(state: State<'_, AppState>, target: AutomationTarget) -> AqlInspection {
    inspect_with_router(state.inner(), target)
}

/// 从共享 Router 生成检查结果，保持 Tauri adapter 只负责参数装配。
fn inspect_with_router(state: &AppState, target: AutomationTarget) -> AqlInspection {
    let query = match &target.locator {
        TargetLocator::Query { query } => query,
        TargetLocator::Visual { .. }
        | TargetLocator::VisualResolved { .. }
        | TargetLocator::Coordinate { .. }
        | TargetLocator::Focused => {
            return AqlInspection::Invalid {
                diagnostics: Vec::new(),
            };
        }
    };
    let document = parse_document(&query.source);
    let Some(parsed) = document.hir else {
        return AqlInspection::Invalid {
            diagnostics: document.diagnostics,
        };
    };
    let analysis = analyze_query(&parsed);
    let action = AutomationAction::Click { target };
    let planning = state.router.inspect_current(&action);

    AqlInspection::Valid {
        canonical_source: analysis.canonical_source().to_owned(),
        portability: analysis.portability().clone(),
        diagnostics: analysis.diagnostics().to_vec(),
        planning,
    }
}
