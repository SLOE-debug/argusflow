//! 每个步骤只推进当前帧；结构节点通过压栈进入子作用域。
use super::{frame::*, runner::Runner, state::EventKind};
use crate::{
    RunError,
    compilation::{Binding, PlanAction},
};
use argusflow_core::OperationOptions;
use argusflow_workflow::{ErrorKind, Value, Values};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

impl Runner {
    pub async fn step(&mut self) -> Result<(), RunError> {
        self.check()?;
        let current = self.current();
        let plan = self.plan.clone();
        let frame = &self.frames[current];
        let scope = &plan.scopes[frame.scope];
        if frame.pc == scope.nodes.len() {
            let outputs = self.eval_fields(&scope.outputs, &Values::new())?;
            self.leave(Signal::Complete(outputs)).await;
            return Ok(());
        }
        if matches!(frame.state, NodeState::Loop { .. }) {
            return self.iterate();
        }
        if !matches!(frame.state, NodeState::Ready) {
            return Err(RunError::new(ErrorKind::Contract, "结构节点等待的子帧缺失"));
        }
        if frame.cleanup_mode {
            self.cleanup_steps += 1;
            if self.cleanup_steps > 1000 {
                return Err(RunError::new(ErrorKind::Limit, "收尾步骤超过一千步"));
            }
        } else {
            self.steps += 1;
            if self.steps > self.options.max_steps {
                return Err(RunError::new(ErrorKind::Limit, "执行步数预算耗尽"));
            }
        }
        let node = &scope.nodes[frame.pc];
        let timeout = node
            .timeout_ms
            .map(Duration::from_millis)
            .unwrap_or_else(|| frame.operation.remaining().min(Duration::from_secs(86_400)));
        if timeout.is_zero() {
            return Err(RunError::new(ErrorKind::Timeout, "节点时限已到"));
        }
        self.frames[current].node_operation = Some(
            self.frames[current]
                .operation
                .child(OperationOptions::new(timeout).map_err(RunError::from)?),
        );
        self.frames[current].execution = Some(self.steps + self.cleanup_steps);
        self.emit(EventKind::NodeStarted);
        match &node.action {
            PlanAction::Let { slot, value } => {
                let value = self.eval(value, &Values::new())?;
                self.assign(
                    vec![(
                        Binding {
                            scope: self.frames[current].scope,
                            slot: *slot,
                        },
                        value,
                    )],
                    true,
                )?;
                self.complete_node(Values::new())
            }
            PlanAction::Assign(assignments) => {
                let assignments = self.eval_assignments(assignments)?;
                self.assign(assignments, false)?;
                self.complete_node(Values::new())
            }
            PlanAction::Block(scope) => self.enter_child(*scope, NodeState::Child, Vec::new()),
            PlanAction::If {
                condition,
                then_scope,
                else_scope,
            } => {
                let value = self.eval(condition, &Values::new())?;
                self.enter_child(
                    if value == Value::Bool(true) {
                        *then_scope
                    } else {
                        *else_scope
                    },
                    NodeState::Child,
                    Vec::new(),
                )
            }
            PlanAction::Switch {
                selector,
                cases,
                default_scope,
            } => {
                let value = self.eval(selector, &Values::new())?;
                let scope = cases
                    .iter()
                    .find(|(case, _)| case == &value)
                    .map_or(*default_scope, |(_, scope)| *scope);
                self.enter_child(scope, NodeState::Child, Vec::new())
            }
            PlanAction::While { .. } => {
                self.frames[current].state = NodeState::Loop {
                    iterations: 0,
                    items: None,
                };
                self.iterate()
            }
            PlanAction::ForEach { items, .. } => {
                let Value::List(items) = self.eval(items, &Values::new())? else {
                    return Err(RunError::new(ErrorKind::Contract, "循环集合不是列表"));
                };
                self.frames[current].state = NodeState::Loop {
                    iterations: 0,
                    items: Some(items),
                };
                if self.data_size() > self.options.data_bytes {
                    return Err(RunError::new(
                        ErrorKind::Limit,
                        "循环集合快照超过总数据预算",
                    ));
                }
                self.iterate()
            }
            PlanAction::Break => {
                self.leave(Signal::Break).await;
                Ok(())
            }
            PlanAction::Continue => {
                self.leave(Signal::Continue).await;
                Ok(())
            }
            PlanAction::Return(values) => {
                let values = self.eval_fields(values, &Values::new())?;
                self.leave(Signal::Return(values)).await;
                Ok(())
            }
            PlanAction::Call {
                scope,
                inputs,
                resources,
            } => {
                let inputs = Arc::new(self.eval_fields(inputs, &Values::new())?);
                let resources = self.resources(resources)?;
                let operation = self.operation().clone();
                self.frames[current].state = NodeState::Call;
                self.push(
                    *scope,
                    0,
                    inputs,
                    resources,
                    Vec::new(),
                    operation,
                    self.frames[current].cleanup_mode,
                )
            }
            PlanAction::Try { body, .. } => {
                if let Err(mut error) = self.enter_child(
                    *body,
                    NodeState::Try {
                        stage: TryStage::Body,
                        pending: None,
                    },
                    Vec::new(),
                ) {
                    error.path = self.path();
                    self.resume_try(TryStage::Body, None, Signal::Error(error))
                        .await
                } else {
                    Ok(())
                }
            }
            PlanAction::Fail(code) => Err(RunError::new(ErrorKind::User, code.clone())),
            PlanAction::Wait(expression) => {
                let Value::Int(ms) = self.eval(expression, &Values::new())? else {
                    return Err(RunError::new(ErrorKind::Contract, "等待时长不是整数"));
                };
                let ms = u64::try_from(ms)
                    .map_err(|_| RunError::new(ErrorKind::Expression, "等待时长不能为负数"))?;
                self.wait_duration(Duration::from_millis(ms)).await?;
                self.complete_node(Values::new())
            }
            PlanAction::Task(task) => {
                let output = self.execute_task(task).await?;
                // 输出映射位于重试之外，外部成功后绝不能因映射失败再执行。
                self.complete_node(output)
                    .map_err(|error| error.with_effect(self.operation().effect()))
            }
            PlanAction::Release(binding) => {
                let index = self.lexical_index(binding.scope)?;
                let lease = self.frames[index]
                    .resources
                    .get(&binding.name)
                    .cloned()
                    .ok_or_else(|| RunError::new(ErrorKind::Stale, "资源已释放"))?;
                let id = lease
                    .id
                    .ok_or_else(|| RunError::new(ErrorKind::Contract, "不能关闭借用资源"))?;
                let operation = self.operation().clone();
                self.pool.close(id, &operation).await?;
                self.frames[index].resources.remove(&binding.name);
                self.frames[index].owned.retain(|owned| *owned != id);
                self.complete_node(Values::new())
            }
        }
    }
    pub fn enter_child(
        &mut self,
        scope: usize,
        state: NodeState,
        variables: Vec<(usize, Value)>,
    ) -> Result<(), RunError> {
        let current = self.current();
        let operation = self.operation().clone();
        let inputs = self.frames[current].inputs.clone();
        let cleanup_mode = self.frames[current].cleanup_mode;
        self.frames[current].state = state;
        self.push(
            scope,
            current,
            inputs,
            BTreeMap::new(),
            variables,
            operation,
            cleanup_mode,
        )
    }
    pub fn iterate(&mut self) -> Result<(), RunError> {
        self.check()?;
        let current = self.current();
        let plan = self.plan.clone();
        let frame = &self.frames[current];
        let NodeState::Loop { iterations, items } = &frame.state else {
            return Err(RunError::new(ErrorKind::Contract, "循环状态缺失"));
        };
        let iterations = *iterations;
        let action = &plan.scopes[frame.scope].nodes[frame.pc].action;
        let (body, limit, variables) = match action {
            PlanAction::While {
                condition,
                body,
                max_iterations,
            } => {
                if self.eval(condition, &Values::new())? == Value::Bool(false) {
                    return self.complete_node(Values::new());
                }
                (
                    *body,
                    max_iterations.unwrap_or(self.options.max_iterations),
                    Vec::new(),
                )
            }
            PlanAction::ForEach {
                body,
                item_slot,
                index_slot,
                max_iterations,
                ..
            } => {
                let items = items
                    .as_ref()
                    .ok_or_else(|| RunError::new(ErrorKind::Contract, "循环集合缺失"))?;
                let Some(value) = items.get(iterations as usize) else {
                    return self.complete_node(Values::new());
                };
                (
                    *body,
                    max_iterations.unwrap_or(self.options.max_iterations),
                    vec![
                        (*item_slot, value.clone()),
                        (*index_slot, Value::Int(i64::from(iterations))),
                    ],
                )
            }
            _ => return Err(RunError::new(ErrorKind::Contract, "错误的循环节点")),
        };
        if iterations >= limit {
            return Err(RunError::new(ErrorKind::Limit, "循环轮数预算耗尽"));
        }
        if let NodeState::Loop { iterations, .. } = &mut self.frames[current].state {
            *iterations += 1;
        }
        let frame = &self.frames[current];
        self.push(
            body,
            current,
            frame.inputs.clone(),
            BTreeMap::new(),
            variables,
            self.operation().clone(),
            frame.cleanup_mode,
        )
    }
    pub async fn wait_duration(&self, duration: Duration) -> Result<(), RunError> {
        let start = std::time::Instant::now();
        loop {
            self.check()?;
            let remaining = duration.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Ok(());
            }
            tokio::time::sleep(
                remaining
                    .min(Duration::from_millis(10))
                    .min(self.operation().remaining()),
            )
            .await;
        }
    }
}
