//! 任务尝试、重试门禁与资源交付；映射失败不进入这里重试。
use super::{guard::guard_future, runner::Runner, state::EventKind};
use crate::{RunError, TaskContext, compilation::PlanTask};
use argusflow_core::{Effect, OperationOptions};
use argusflow_workflow::{ErrorKind, Values};
use std::{collections::BTreeMap, time::Duration};

impl Runner {
    pub async fn execute_task(&mut self, task: &PlanTask) -> Result<Values, RunError> {
        let inputs = self.eval_fields(&task.inputs, &Values::new())?;
        let leases = self.resources(&task.resources)?;
        let resources = leases
            .iter()
            .map(|(name, lease)| (name.clone(), lease.resource.clone()))
            .collect::<BTreeMap<_, _>>();
        let dependencies = leases.values().filter_map(|lease| lease.id).collect();
        let attempts = task.retry.as_ref().map_or(1, |retry| retry.max_attempts);
        for attempt in 1..=attempts {
            self.check()?;
            let permit = self.pool.reserve(task.signature.resource_outputs.len())?;
            let operation = self.operation().child(
                OperationOptions::new(self.operation().remaining()).map_err(RunError::from)?,
            );
            let context = TaskContext {
                inputs: &inputs,
                resources: &resources,
                operation: &operation,
            };
            let result = {
                let future = guard_future(async { task.task.execute(context).await });
                tokio::pin!(future);
                loop {
                    tokio::select! {
                        biased;
                        output = &mut future => break output,
                        _ = tokio::time::sleep(Duration::from_millis(10)) => {
                            if let Err(error) = self.check() { operation.cancel(); break Err(error.with_effect(operation.effect())); }
                        }
                    }
                }
            };
            match result {
                Ok(output) => {
                    // 即使扩展违反输出契约，也先接管所有资源，保证失败路径仍然清理。
                    let adopted = self
                        .pool
                        .adopt(self.id, output.resources, dependencies, permit)
                        .await;
                    let current = self.current();
                    self.frames[current]
                        .owned
                        .extend(adopted.values().filter_map(|lease| lease.id));
                    if output.values.keys().ne(task.signature.outputs.keys())
                        || output.values.iter().any(|(name, value)| {
                            !value.matches(&task.signature.outputs[name])
                                || value
                                    .measured_bytes()
                                    .is_none_or(|n| n > self.options.value_bytes)
                        })
                        || adopted.keys().ne(task.signature.resource_outputs.keys())
                        || adopted.iter().any(|(port, lease)| {
                            lease.resource.resource_type() != task.signature.resource_outputs[port]
                        })
                    {
                        return Err(RunError::new(
                            ErrorKind::Contract,
                            "任务结果违反端口、类型或数据预算契约",
                        )
                        .with_effect(operation.effect()));
                    }
                    for (port, lease) in adopted {
                        self.frames[current]
                            .resources
                            .insert(task.resource_outputs[&port].clone(), lease);
                    }
                    if operation.effect() == Effect::Unconfirmed {
                        self.operation()
                            .begin_effect("workflow_task_completed")
                            .map_err(|error| {
                                RunError::from(error).with_effect(operation.effect())
                            })?;
                    }
                    self.check()
                        .map_err(|error| error.with_effect(operation.effect()))?;
                    return Ok(output.values);
                }
                Err(mut error) => {
                    if operation.effect() == Effect::Unconfirmed {
                        error.effect = Effect::Unconfirmed;
                    }
                    let retry = task.retry.as_ref().filter(|retry| {
                        task.signature.safe_to_retry
                            && attempt < attempts
                            && error.effect == Effect::None
                            && error.catchable()
                            && retry.errors.contains(&error.kind)
                    });
                    let Some(retry) = retry else {
                        return Err(error);
                    };
                    let delay = retry
                        .initial_delay_ms
                        .saturating_mul(1u64 << (attempt - 1))
                        .min(retry.max_delay_ms);
                    self.wait_duration(Duration::from_millis(delay)).await?;
                    self.emit(EventKind::Retrying {
                        attempt: attempt + 1,
                    });
                }
            }
        }
        Err(RunError::new(ErrorKind::Contract, "任务没有执行任何尝试"))
    }
}
