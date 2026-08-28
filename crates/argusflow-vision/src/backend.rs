//! VisualCache/OcrTiny/OcrMedium backend 与 PreparedPlan 的接入。

use std::{collections::BTreeMap, sync::Arc, time::Instant};

use argusflow_agent::{
    ActionBackend, ContextFitness, ExecutionContext, PlanExplain, PlanRejection, PlanStepExplain,
    PlanStepKind, PreparedCandidate, PreparedExecution, RuntimeAvailability,
};
use argusflow_core::{
    ActionCapability, ActionOutcome, AutomationAction, AutomationError, BackendKind,
    ExtractCardinality, FieldProjectionSource, TargetLocator, VisualQuery,
};
use argusflow_query::{BranchPath, QueryCost, QueryPortability, SupportLevel};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::{
    evidence::VisionPreparedDiagnostics,
    query::{VisualMatch, evaluate_visual_query, matching_nodes},
    runtime::{SceneRefreshPolicy, VisionRuntime},
};

/// 共享 VisionRuntime 的视觉观察 backend。
#[derive(Debug, Clone)]
pub struct VisionBackend {
    /// 实际负责 capture/OCR/cache 的共享 runtime。
    runtime: Arc<VisionRuntime>,
    /// 该实例暴露的 backend 层级。
    kind: BackendKind,
}

impl VisionBackend {
    /// 创建指定 backend 层级的视觉后端。
    pub fn new(runtime: Arc<VisionRuntime>, kind: BackendKind) -> Self {
        Self { runtime, kind }
    }

    /// 返回绑定的 runtime。
    pub fn runtime(&self) -> Arc<VisionRuntime> {
        self.runtime.clone()
    }
}

impl ActionBackend for VisionBackend {
    fn kind(&self) -> BackendKind {
        self.kind
    }

    fn prepare(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        let TargetLocator::Visual { query } = &action.target().locator else {
            return Err(PlanRejection::Unsupported { backend: self.kind });
        };
        if !supports_observation(action) || !supports_extract(action) {
            return Err(PlanRejection::Unsupported { backend: self.kind });
        }
        let window = context.foreground_window.clone();
        let Some(window_ref) = window.as_ref() else {
            return Ok(vec![self.candidate(
                action,
                query,
                None,
                RuntimeAvailability::MissingContext,
            )]);
        };
        let health = self.runtime.health();
        let availability = match self.kind {
            BackendKind::VisualCache => {
                if health.capture_ready {
                    RuntimeAvailability::Ready
                } else {
                    RuntimeAvailability::Unavailable
                }
            }
            BackendKind::OcrTiny | BackendKind::OcrMedium => {
                if health.capture_ready && health.worker_ready {
                    RuntimeAvailability::Ready
                } else if !health.capture_ready {
                    RuntimeAvailability::Unavailable
                } else {
                    RuntimeAvailability::Unavailable
                }
            }
            _ => RuntimeAvailability::NotImplemented,
        };
        Ok(vec![self.candidate(
            action,
            query,
            Some(window_ref),
            availability,
        )])
    }
}

impl VisionBackend {
    /// 构造一个绑定了 prepare 事实的候选。
    fn candidate(
        &self,
        action: &AutomationAction,
        query: &VisualQuery,
        prepared_window: Option<&argusflow_agent::WindowContext>,
        availability: RuntimeAvailability,
    ) -> PreparedCandidate {
        let cost = match self.kind {
            BackendKind::VisualCache => QueryCost::Low,
            BackendKind::OcrTiny => QueryCost::Medium,
            BackendKind::OcrMedium => QueryCost::High,
            _ => QueryCost::High,
        };
        let explain = PlanExplain {
            backend: self.kind,
            branch_path: Some(BranchPath::root()),
            support: SupportLevel::Native,
            cost,
            availability,
            context_fitness: if prepared_window.is_some() {
                ContextFitness::Good
            } else {
                ContextFitness::Poor
            },
            portability: QueryPortability::Portable,
            steps: vec![
                PlanStepExplain {
                    kind: PlanStepKind::Scope,
                    summary: "frozen AppSession HWND/PID visual scope".to_owned(),
                },
                PlanStepExplain {
                    kind: if self.kind == BackendKind::VisualCache {
                        PlanStepKind::Cache
                    } else {
                        PlanStepKind::CandidateSource
                    },
                    summary: format!("{} query: {:?}", self.kind_name(), query.text),
                },
                PlanStepExplain {
                    kind: PlanStepKind::Selection,
                    summary: if query.exact {
                        "exact query requires 0/1/N evaluation".to_owned()
                    } else {
                        "contains query returns explicit candidate set".to_owned()
                    },
                },
            ],
            diagnostics: Vec::new(),
        };
        let window = prepared_window.cloned();
        let diagnostics = window.as_ref().map(|window| {
            Arc::new(VisionPreparedDiagnostics::new(
                self.runtime.clone(),
                argusflow_core::WindowIdentity {
                    handle: window.handle,
                    process_id: window.process_id,
                },
                query.text.clone(),
                self.kind,
            )) as Arc<dyn argusflow_agent::PreparedDiagnostics>
        });
        let execution = VisionPreparedExecution {
            backend: self.kind,
            runtime: self.runtime.clone(),
            query: query.clone(),
            window,
            action: action.clone(),
        };
        let candidate = PreparedCandidate::new(explain, Arc::new(execution));
        if let Some(diagnostics) = diagnostics {
            candidate.with_diagnostics(diagnostics)
        } else {
            candidate
        }
    }

    /// 返回后端的稳定人类可读名称。
    fn kind_name(&self) -> &'static str {
        match self.kind {
            BackendKind::VisualCache => "visual-cache",
            BackendKind::OcrTiny => "ocr-tiny",
            BackendKind::OcrMedium => "ocr-medium",
            _ => "unsupported-visual",
        }
    }
}

/// 绑定窗口身份和视觉查询的执行计划。
#[derive(Debug)]
struct VisionPreparedExecution {
    /// 实际 backend 层级。
    backend: BackendKind,
    /// 共享 runtime。
    runtime: Arc<VisionRuntime>,
    /// prepare 阶段冻结的查询。
    query: VisualQuery,
    /// prepare 阶段冻结的窗口句柄；PID 由 scope 校验补齐。
    window: Option<argusflow_agent::WindowContext>,
    /// prepare 阶段冻结的动作类型和字段。
    action: AutomationAction,
}

#[async_trait]
impl PreparedExecution for VisionPreparedExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| AutomationError::BackendUnavailable {
                backend: self.backend,
                message: "visual execution has no frozen window context".to_owned(),
            })?;
        let scene = match self.backend {
            BackendKind::VisualCache => match self.runtime.lookup_cache(
                argusflow_core::WindowIdentity {
                    handle: window.handle,
                    process_id: window.process_id,
                },
                &SceneRefreshPolicy::tiny(),
            ) {
                crate::scene::CacheLookup::Hit(scene) => scene,
                crate::scene::CacheLookup::Miss(reason) => {
                    return Err(AutomationError::BackendUnavailable {
                        backend: self.backend,
                        message: format!("visual cache miss: {reason:?}"),
                    });
                }
            },
            BackendKind::OcrTiny => self
                .runtime
                .current_scene(
                    argusflow_core::WindowIdentity {
                        handle: window.handle,
                        process_id: window.process_id,
                    },
                    &SceneRefreshPolicy::tiny(),
                )
                .await
                .map_err(|error| vision_error_to_automation(self.backend, error))?,
            BackendKind::OcrMedium => self
                .runtime
                .current_scene(
                    argusflow_core::WindowIdentity {
                        handle: window.handle,
                        process_id: window.process_id,
                    },
                    &SceneRefreshPolicy::medium(),
                )
                .await
                .map_err(|error| vision_error_to_automation(self.backend, error))?,
            _ => {
                return Err(AutomationError::BackendUnavailable {
                    backend: self.backend,
                    message: "unsupported visual backend".to_owned(),
                });
            }
        };
        self.runtime.metrics().record_scene_query();
        let query_started_at = Instant::now();
        let result = execute_observation(&self.action, &scene, &self.query, self.backend);
        self.runtime
            .metrics()
            .record_scene_query_latency(query_started_at.elapsed());
        result
    }
}

/// 当前 P0 只负责观察和读取，物理 click/set value 留给 SendInput/UIA。
fn supports_observation(action: &AutomationAction) -> bool {
    matches!(
        action,
        AutomationAction::GetText { .. }
            | AutomationAction::GetValue { .. }
            | AutomationAction::Extract { .. }
    )
}

/// 检查 Extract 字段是否能由视觉文本事实表达。
fn supports_extract(action: &AutomationAction) -> bool {
    let AutomationAction::Extract { fields, .. } = action else {
        return true;
    };
    fields.iter().all(|field| {
        matches!(
            &field.source,
            FieldProjectionSource::Text | FieldProjectionSource::Name
        )
    })
}

/// 把视觉观察结果转换为现有 ActionOutcome。
fn execute_observation(
    action: &AutomationAction,
    scene: &crate::scene::VisualScene,
    query: &VisualQuery,
    backend: BackendKind,
) -> Result<ActionOutcome, AutomationError> {
    match action {
        AutomationAction::GetText { .. } | AutomationAction::GetValue { .. } => {
            let VisualMatch::Unique(node) = evaluate_visual_query(scene, query)?;
            let mut outputs = BTreeMap::new();
            outputs.insert("text".to_owned(), Value::String(node.raw_text.clone()));
            Ok(ActionOutcome {
                backend,
                message: "已从视觉场景读取目标文本".to_owned(),
                outputs,
                diagnostic_evidence: Vec::new(),
            })
        }
        AutomationAction::Extract {
            cardinality,
            fields,
            ..
        } => {
            let nodes = match cardinality {
                ExtractCardinality::One => {
                    let VisualMatch::Unique(node) = evaluate_visual_query(scene, query)?;
                    vec![node]
                }
                ExtractCardinality::Many => matching_nodes(scene, query),
            };
            let values = nodes
                .iter()
                .map(|node| project_fields(node, fields, backend))
                .collect::<Result<Vec<_>, _>>()?;
            let mut outputs = BTreeMap::new();
            outputs.insert(
                "value".to_owned(),
                if *cardinality == ExtractCardinality::One {
                    values.into_iter().next().unwrap_or_else(|| json!({}))
                } else {
                    Value::Array(values)
                },
            );
            Ok(ActionOutcome {
                backend,
                message: "已从视觉场景提取结构化文本".to_owned(),
                outputs,
                diagnostic_evidence: Vec::new(),
            })
        }
        _ => Err(AutomationError::BackendUnavailable {
            backend,
            message: "视觉 P0 只提供观察动作，未执行物理输入".to_owned(),
        }),
    }
}

/// 把一个视觉节点投影成 Extract 字段对象。
fn project_fields(
    node: &crate::scene::VisualNode,
    fields: &[argusflow_core::FieldProjection],
    backend: BackendKind,
) -> Result<Value, AutomationError> {
    let mut object = serde_json::Map::new();
    for field in fields {
        let value = match &field.source {
            FieldProjectionSource::Text | FieldProjectionSource::Name => {
                Value::String(node.raw_text.clone())
            }
            FieldProjectionSource::Value
            | FieldProjectionSource::Property { .. }
            | FieldProjectionSource::Attribute { .. } => {
                return Err(AutomationError::ActionUnsupported {
                    backend,
                    query: node.raw_text.clone(),
                    semantic_matches: 1,
                    required: ActionCapability::ReadValue,
                });
            }
        };
        object.insert(field.name.clone(), value);
    }
    Ok(Value::Object(object))
}

/// 将视觉内部错误映射为现有 Planner 能理解的错误。
fn vision_error_to_automation(
    backend: BackendKind,
    error: crate::error::VisionError,
) -> AutomationError {
    match error {
        crate::error::VisionError::WorkerUnavailable { message }
        | crate::error::VisionError::CaptureUnavailable { message }
        | crate::error::VisionError::OcrFailed { message }
        | crate::error::VisionError::Protocol { message } => {
            AutomationError::BackendUnavailable { backend, message }
        }
        crate::error::VisionError::FrameTimeout { timeout_ms } => AutomationError::BackendFailed {
            backend,
            message: format!("visual frame timed out after {timeout_ms}ms"),
        },
        other => AutomationError::BackendFailed {
            backend,
            message: other.to_string(),
        },
    }
}
