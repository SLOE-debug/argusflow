//! 单一 Vision Backend：进程窗口枚举、Small OCR 与 Medium 内部升级。

use std::{collections::BTreeMap, sync::Arc};

use argusflow_agent::{
    ActionBackend, ContextFitness, ExecutionContext, PlanExplain, PlanRejection, PlanStepExplain,
    PlanStepKind, PreparedCandidate, PreparedExecution, RuntimeAvailability,
};
use argusflow_core::{
    ActionCapability, ActionOutcome, ActionOutputKey, AutomationAction, AutomationError,
    BackendKind, ExtractCardinality, FieldProjection, FieldProjectionSource,
    PreparedAutomationTarget, PreparedTargetLocator, TargetLocator, VisualQuery,
};
use argusflow_query::{BranchPath, QueryCost, QueryPortability, SupportLevel};
use async_trait::async_trait;
use serde_json::Value;

use crate::{OcrProfile, SceneRefreshPolicy, VisionRuntime, WindowInventory, matching_app_nodes};

/// 对外只暴露一个候选，OCR 档位升级完全封装在 VisionRuntime 内部。
#[derive(Debug, Clone)]
pub struct VisionBackend {
    /// 捕获、增量 Scene 和 OCR 策略运行时。
    runtime: Arc<VisionRuntime>,
    /// 平台窗口注册表。
    inventory: Arc<dyn WindowInventory>,
}

impl VisionBackend {
    /// 创建绑定共享 Runtime 与 Windows 窗口注册表的视觉读取后端。
    pub fn new(runtime: Arc<VisionRuntime>, inventory: Arc<dyn WindowInventory>) -> Self {
        Self { runtime, inventory }
    }

    /// 返回共享 Runtime。
    pub fn runtime(&self) -> Arc<VisionRuntime> {
        self.runtime.clone()
    }
}

impl ActionBackend for VisionBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::OcrSmall
    }

    fn prepare(
        &self,
        _action: &AutomationAction,
        _context: &ExecutionContext,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        Err(PlanRejection::Unsupported {
            backend: BackendKind::OcrSmall,
        })
    }

    fn prepare_with_target(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
        prepared_target: Option<&PreparedAutomationTarget>,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        if !supports_action(action)
            || !matches!(action.target().locator, TargetLocator::Visual { .. })
        {
            return Err(PlanRejection::Unsupported {
                backend: BackendKind::OcrSmall,
            });
        }
        let Some(PreparedTargetLocator::Visual { query }) =
            prepared_target.map(PreparedAutomationTarget::locator)
        else {
            return Err(PlanRejection::Unsupported {
                backend: BackendKind::OcrSmall,
            });
        };
        let window = context.foreground_window.clone();
        let health = self.runtime.health();
        let availability = if window.is_none() {
            RuntimeAvailability::MissingContext
        } else if health.capture_ready && health.worker_ready {
            RuntimeAvailability::Ready
        } else {
            RuntimeAvailability::Unavailable
        };
        let explain = PlanExplain {
            backend: BackendKind::OcrSmall,
            branch_path: Some(BranchPath::root()),
            support: SupportLevel::Native,
            cost: QueryCost::Medium,
            availability,
            context_fitness: if window.is_some() {
                ContextFitness::Good
            } else {
                ContextFitness::Poor
            },
            portability: QueryPortability::Portable,
            steps: vec![
                PlanStepExplain {
                    kind: PlanStepKind::Scope,
                    summary: "enumerate visible top-level windows for the frozen process"
                        .to_owned(),
                },
                PlanStepExplain {
                    kind: PlanStepKind::CandidateSource,
                    summary: "reuse unchanged WindowScene or refresh dirty regions with Small OCR"
                        .to_owned(),
                },
                PlanStepExplain {
                    kind: PlanStepKind::Selection,
                    summary: "upgrade unresolved text to Medium and enforce 0/1/N".to_owned(),
                },
            ],
            diagnostics: Vec::new(),
        };
        Ok(vec![PreparedCandidate::new(
            explain,
            Arc::new(VisionExecution {
                runtime: self.runtime.clone(),
                inventory: self.inventory.clone(),
                window,
                query: query.clone(),
                action: action.clone(),
            }),
        )])
    }
}

/// Prepare 后冻结的视觉读取执行。
#[derive(Debug)]
struct VisionExecution {
    /// 共享视觉运行时。
    runtime: Arc<VisionRuntime>,
    /// 平台窗口注册表。
    inventory: Arc<dyn WindowInventory>,
    /// 冻结的进程/前台窗口上下文。
    window: Option<argusflow_agent::WindowContext>,
    /// 文本与可选区域查询。
    query: VisualQuery,
    /// 输出契约所属动作。
    action: AutomationAction,
}

#[async_trait]
impl PreparedExecution for VisionExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| AutomationError::BackendUnavailable {
                backend: BackendKind::OcrSmall,
                message: "Vision execution has no frozen process context".to_owned(),
            })?;
        match &self.action {
            AutomationAction::GetText { .. } => {
                let target = self
                    .runtime
                    .resolve_text(
                        self.inventory.as_ref(),
                        window.process_id,
                        &self.query,
                        0.35,
                        None,
                    )
                    .await?;
                let outputs = BTreeMap::from([(
                    ActionOutputKey::Text.as_str().to_owned(),
                    Value::String(target.node.raw_text),
                )]);
                outcome(&self.action, outputs, "Vision text query resolved uniquely")
            }
            AutomationAction::Extract {
                cardinality,
                fields,
                ..
            } => self.extract(window.process_id, *cardinality, fields).await,
            _ => Err(AutomationError::ActionUnsupported {
                backend: BackendKind::OcrSmall,
                query: self.query.text.clone(),
                semantic_matches: 0,
                required: ActionCapability::ReadText,
            }),
        }
    }
}

impl VisionExecution {
    /// 执行单项或多项文本字段提取；空结果会用 Medium 再观察一次。
    async fn extract(
        &self,
        process_id: u32,
        cardinality: ExtractCardinality,
        fields: &[FieldProjection],
    ) -> Result<ActionOutcome, AutomationError> {
        if cardinality == ExtractCardinality::One {
            let target = self
                .runtime
                .resolve_text(self.inventory.as_ref(), process_id, &self.query, 0.35, None)
                .await?;
            let outputs = BTreeMap::from([(
                ActionOutputKey::Item.as_str().to_owned(),
                project_fields(&target.node, fields)?,
            )]);
            return outcome(&self.action, outputs, "Vision extracted one text node");
        }

        let small = self
            .runtime
            .current_app_scene(
                self.inventory.as_ref(),
                process_id,
                &SceneRefreshPolicy::small(),
                None,
            )
            .await
            .map_err(map_runtime_error)?;
        let selected_scene = if matching_app_nodes(&small, &self.query).is_empty() {
            let mut policy = SceneRefreshPolicy::medium();
            policy.force_full_ocr = true;
            let medium = self
                .runtime
                .current_app_scene(self.inventory.as_ref(), process_id, &policy, None)
                .await
                .map_err(map_runtime_error)?;
            if matching_app_nodes(&medium, &self.query).is_empty() {
                let mut binary_policy = SceneRefreshPolicy::medium();
                binary_policy.force_full_ocr = true;
                binary_policy.ocr = OcrProfile::medium_binary();
                self.runtime
                    .current_app_scene(self.inventory.as_ref(), process_id, &binary_policy, None)
                    .await
                    .map_err(map_runtime_error)?
            } else {
                medium
            }
        } else {
            small
        };
        let matches = matching_app_nodes(&selected_scene, &self.query);
        let values = matches
            .iter()
            .map(|candidate| project_fields(candidate.node, fields))
            .collect::<Result<Vec<_>, _>>()?;
        let outputs = BTreeMap::from([(
            ActionOutputKey::Items.as_str().to_owned(),
            Value::Array(values),
        )]);
        outcome(
            &self.action,
            outputs,
            "Vision extracted matching text nodes",
        )
    }
}

/// 当前 MVP 只读取文字事实。
fn supports_action(action: &AutomationAction) -> bool {
    match action {
        AutomationAction::GetText { .. } => true,
        AutomationAction::Extract { fields, .. } => fields.iter().all(|field| {
            matches!(
                field.source,
                FieldProjectionSource::Text | FieldProjectionSource::Name
            )
        }),
        _ => false,
    }
}

/// 将文本节点投影成现有 Extract 对象。
fn project_fields(
    node: &crate::VisualNode,
    fields: &[FieldProjection],
) -> Result<Value, AutomationError> {
    let mut object = serde_json::Map::new();
    for field in fields {
        let value = match field.source {
            FieldProjectionSource::Text | FieldProjectionSource::Name => {
                Value::String(node.raw_text.clone())
            }
            _ => {
                return Err(AutomationError::ActionUnsupported {
                    backend: BackendKind::OcrSmall,
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

/// 校验并构造统一动作输出。
fn outcome(
    action: &AutomationAction,
    outputs: BTreeMap<String, Value>,
    message: &str,
) -> Result<ActionOutcome, AutomationError> {
    action
        .output_contract()
        .validate(&outputs)
        .map_err(|error| AutomationError::BackendFailed {
            backend: BackendKind::OcrSmall,
            message: format!("Vision output contract mismatch: {error:?}"),
        })?;
    Ok(ActionOutcome {
        backend: BackendKind::OcrSmall,
        message: message.to_owned(),
        outputs,
        diagnostic_evidence: Vec::new(),
    })
}

/// 将内部捕获/OCR错误映射到 Planner 运行错误。
fn map_runtime_error(error: crate::VisionError) -> AutomationError {
    match error {
        crate::VisionError::CaptureUnavailable { message }
        | crate::VisionError::WorkerUnavailable { message }
        | crate::VisionError::OcrFailed { message }
        | crate::VisionError::Protocol { message } => AutomationError::BackendUnavailable {
            backend: BackendKind::OcrSmall,
            message,
        },
        other => AutomationError::BackendFailed {
            backend: BackendKind::OcrSmall,
            message: other.to_string(),
        },
    }
}
