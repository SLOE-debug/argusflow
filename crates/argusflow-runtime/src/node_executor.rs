use std::{collections::BTreeMap, sync::Arc, time::Duration};

use argusflow_core::{
    ApplicationSessionProvider, AutomationAction, AutomationExecutionScope, ExecutionEventKind,
    ExecutionEventPayload, ResourceRef, TargetScope, UiOperation, WorkflowCapability, WorkflowNode,
    WorkflowNodeKind, WorkflowPermissions,
};

use crate::{ActionDispatcher, CommandExecutor, NodeOutcome, RunContext, RuntimeError};

/// 节点执行器交给 Engine 发出的单个节点内事件。
#[derive(Debug)]
pub(crate) struct NodeEvent {
    /// 节点内事件类别。
    pub(crate) kind: ExecutionEventKind,
    /// 可选说明；只有用户显式放置的 Debug 节点会在这里携带业务文本。
    pub(crate) message: Option<String>,
    /// 可安全传给前端的结构化载荷。
    pub(crate) payload: Option<ExecutionEventPayload>,
}

/// 一个节点完成后的结构化结果与可观察事件。
#[derive(Debug, Default)]
pub(crate) struct NodeExecution {
    /// 保存到 RunContext 的值和资源端口。
    pub(crate) outcome: NodeOutcome,
    /// 在 NodeSucceeded 前按顺序发出的节点内事件。
    pub(crate) events: Vec<NodeEvent>,
}

/// 按节点语义协调资源、命令和 UI 执行边界。
pub(crate) struct WorkflowNodeExecutor {
    /// 只负责语义 UI 动作后端规划与执行。
    dispatcher: Arc<dyn ActionDispatcher>,
    /// 获取和清理平台应用会话。
    applications: Arc<dyn ApplicationSessionProvider>,
    /// 独立于 UI Planner 的命令执行器。
    commands: CommandExecutor,
}

impl WorkflowNodeExecutor {
    /// 使用宿主注入的 UI 和应用资源能力创建节点执行器。
    pub(crate) fn new(
        dispatcher: Arc<dyn ActionDispatcher>,
        applications: Arc<dyn ApplicationSessionProvider>,
    ) -> Self {
        Self {
            dispatcher,
            applications,
            commands: CommandExecutor,
        }
    }

    /// 执行单个节点并直接更新本次运行上下文中的资源绑定。
    pub(crate) async fn execute(
        &self,
        node: &WorkflowNode,
        permissions: WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        match &node.kind {
            WorkflowNodeKind::Start
            | WorkflowNodeKind::End
            | WorkflowNodeKind::Condition { .. } => Ok(NodeExecution::default()),
            WorkflowNodeKind::Log { message } => Ok(NodeExecution {
                outcome: NodeOutcome::default(),
                events: vec![NodeEvent {
                    kind: ExecutionEventKind::Log,
                    message: Some(message.clone()),
                    payload: None,
                }],
            }),
            WorkflowNodeKind::Debug { value } => Ok(NodeExecution {
                outcome: NodeOutcome::default(),
                events: vec![NodeEvent {
                    kind: ExecutionEventKind::Log,
                    // Debug 节点由用户显式放置，因此允许把解析值写入开发日志。
                    message: Some(context.resolve_text(value)?),
                    payload: None,
                }],
            }),
            WorkflowNodeKind::Delay { milliseconds } => {
                tokio::time::sleep(Duration::from_millis(*milliseconds)).await;
                Ok(NodeExecution::default())
            }
            WorkflowNodeKind::Application { spec } => {
                if spec.acquire_policy.may_launch()
                    && !permissions.allows(WorkflowCapability::ApplicationLaunch)
                {
                    return Err(RuntimeError::CapabilityDenied {
                        capability: WorkflowCapability::ApplicationLaunch,
                    });
                }
                let session = self.applications.acquire(spec).await?;
                let output_name = "session".to_owned();
                context.resources_mut().insert_application(
                    ResourceRef {
                        producer_node_id: node.id.clone(),
                        output_name: output_name.clone(),
                    },
                    session,
                );
                Ok(NodeExecution {
                    outcome: NodeOutcome {
                        outputs: BTreeMap::new(),
                        resources: vec![output_name.clone()],
                    },
                    events: vec![NodeEvent {
                        kind: ExecutionEventKind::ResourceAcquired,
                        message: Some("应用会话已获取".to_owned()),
                        payload: Some(ExecutionEventPayload::ResourceAcquired {
                            output_name,
                            resource_type: "application".to_owned(),
                        }),
                    }],
                })
            }
            WorkflowNodeKind::Ui { operation } => self.execute_ui(operation, context).await,
            WorkflowNodeKind::Command { operation } => {
                let outcome = self
                    .commands
                    .execute(operation, permissions, context)
                    .await?;
                let exit_code = outcome
                    .outputs
                    .get("exit_code")
                    .and_then(serde_json::Value::as_i64)
                    .and_then(|value| i32::try_from(value).ok())
                    .ok_or_else(|| {
                        RuntimeError::ExecutionInvariant(
                            "command outcome did not contain an i32 exit_code".to_owned(),
                        )
                    })?;
                let output_names = outcome.outputs.keys().cloned().collect::<Vec<_>>();
                Ok(NodeExecution {
                    outcome,
                    events: vec![
                        NodeEvent {
                            kind: ExecutionEventKind::CommandExited,
                            message: Some(format!("命令执行完成，退出代码 {exit_code}")),
                            payload: Some(ExecutionEventPayload::CommandExited { exit_code }),
                        },
                        NodeEvent {
                            kind: ExecutionEventKind::NodeOutputProduced,
                            message: Some(format!("已产生 {} 个值输出", output_names.len())),
                            payload: Some(ExecutionEventPayload::NodeOutputsProduced {
                                output_names,
                            }),
                        },
                    ],
                })
            }
        }
    }

    /// 解析 UI 节点的数据和资源引用后，交给 ActionDispatcher 选择后端。
    async fn execute_ui(
        &self,
        operation: &UiOperation,
        context: &RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let target = operation.target().clone();
        let scope = resolve_execution_scope(&target.scope, context)?;
        let action = match operation {
            UiOperation::Click { .. } => AutomationAction::Click { target },
            UiOperation::SetValue { value, .. } => AutomationAction::SetValue {
                target,
                value: context.resolve_text(value)?,
            },
            UiOperation::GetText { .. } => AutomationAction::GetText { target },
            UiOperation::GetValue { .. } => AutomationAction::GetValue { target },
        };
        let action_outcome = self.dispatcher.execute(&action, scope).await?;
        let output_names = action_outcome.outputs.keys().cloned().collect::<Vec<_>>();
        let mut events = vec![NodeEvent {
            kind: ExecutionEventKind::BackendSelected,
            message: Some(action_outcome.message),
            payload: Some(ExecutionEventPayload::BackendSelected {
                backend: action_outcome.backend,
            }),
        }];
        if !output_names.is_empty() {
            events.push(NodeEvent {
                kind: ExecutionEventKind::NodeOutputProduced,
                message: Some(format!("已产生 {} 个值输出", output_names.len())),
                payload: Some(ExecutionEventPayload::NodeOutputsProduced { output_names }),
            });
        }
        Ok(NodeExecution {
            outcome: NodeOutcome::values(action_outcome.outputs),
            events,
        })
    }

    /// 按反向获取顺序执行应用资源清理策略。
    pub(crate) async fn cleanup(&self, context: &RunContext) -> Result<(), RuntimeError> {
        for session in context.resources().applications_for_cleanup() {
            self.applications.cleanup(&session).await?;
        }
        Ok(())
    }
}

/// 把 TargetScope 解析成不会泄露到工作流定义的瞬时后端作用域。
fn resolve_execution_scope(
    scope: &TargetScope,
    context: &RunContext,
) -> Result<AutomationExecutionScope, RuntimeError> {
    match scope {
        TargetScope::Current => Ok(AutomationExecutionScope::Current),
        TargetScope::Application { resource } => {
            let session = context.resources().application(resource)?;
            let [window] = session.windows.as_slice() else {
                return Err(RuntimeError::ExecutionInvariant(format!(
                    "application resource '{}.{}' does not contain exactly one window",
                    resource.producer_node_id, resource.output_name,
                )));
            };
            Ok(AutomationExecutionScope::Window {
                handle: window.handle,
                process_id: window.process_id,
                capabilities: session.capabilities,
            })
        }
    }
}
