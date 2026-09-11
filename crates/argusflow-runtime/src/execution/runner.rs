//! 显式栈驱动执行；退出路径在弹出帧之前回收资源。
use super::{api::RunInputs, frame::*, state::*};
use crate::{ExecutionLocation, PreparedWorkflow, ResourcePool, RunError, resource::Lease};
use argusflow_core::{Operation, OperationOptions};
use argusflow_workflow::{ErrorKind, Value, Values};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::broadcast;

pub(super) struct Runner {
    pub id: u64,
    pub frames: Vec<Frame>,
    pub options: RunOptions,
    pub root_operation: Operation,
    pub pool: Arc<ResourcePool>,
    pub events: broadcast::Sender<ExecutionEvent>,
    pub sequence: Arc<AtomicU64>,
    pub status: tokio::sync::watch::Sender<(RunStatus, Option<Arc<RunResult>>)>,
    pub next_instance: u64,
    pub steps: u64,
    pub cleanup_steps: u64,
    pub incoming: Option<Signal>,
    pub unwind_operation: Option<Operation>,
}
impl Runner {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u64,
        plan: Arc<PreparedWorkflow>,
        inputs: RunInputs,
        options: RunOptions,
        operation: Operation,
        pool: Arc<ResourcePool>,
        events: broadcast::Sender<ExecutionEvent>,
        sequence: Arc<AtomicU64>,
        status: tokio::sync::watch::Sender<(RunStatus, Option<Arc<RunResult>>)>,
    ) -> Self {
        let scope = &plan.scopes[plan.root];
        let frame = Frame {
            plan: plan.clone(),
            workflow_root: 0,
            scope: plan.root,
            instance: 1,
            lexical_parent: None,
            inputs: Arc::new(inputs.values),
            variables: vec![None; scope.slots.len()],
            outputs: vec![None; scope.nodes.len()],
            resources: inputs
                .resources
                .into_iter()
                .map(|(name, resource)| (name, Lease { id: None, resource }))
                .collect(),
            owned: Vec::new(),
            pc: 0,
            execution: None,
            state: NodeState::Ready,
            operation: operation.clone(),
            node_operation: None,
            cleanup_mode: false,
        };
        Self {
            id,
            frames: vec![frame],
            options,
            root_operation: operation,
            pool,
            events,
            sequence,
            status,
            next_instance: 2,
            steps: 0,
            cleanup_steps: 0,
            incoming: None,
            unwind_operation: None,
        }
    }
    pub async fn run(mut self) -> Result<Values, RunError> {
        self.emit(EventKind::RunStarted);
        self.emit(EventKind::ScopeEntered);
        loop {
            if let Some(signal) = self.incoming.take() {
                if self.frames.is_empty() {
                    return match signal {
                        Signal::Complete(values) | Signal::Return(values) => Ok(values),
                        Signal::Error(error) => Err(error),
                        _ => Err(RunError::new(ErrorKind::Contract, "控制转移逃出根作用域")),
                    };
                }
                if let Err(error) = self.resume(signal).await {
                    self.leave(Signal::Error(error)).await;
                }
                continue;
            }
            if let Err(error) = self.step().await {
                self.leave(Signal::Error(error)).await;
            }
            // 长时间纯计算也必须给取消控制方和其他运行机会。
            if self.steps.is_multiple_of(64) {
                tokio::task::yield_now().await;
            }
        }
    }
    pub fn current(&self) -> usize {
        self.frames.len() - 1
    }
    pub fn path(&self) -> Vec<ExecutionLocation> {
        self.frames
            .iter()
            .map(|frame| ExecutionLocation {
                workflow: frame.plan.identity.clone(),
                scope: frame.plan.scopes[frame.scope].id.clone(),
                instance: frame.instance,
                node: frame.plan.scopes[frame.scope]
                    .nodes
                    .get(frame.pc)
                    .map(|node| node.id.clone()),
                execution: frame.execution,
            })
            .collect()
    }
    pub fn emit(&self, kind: EventKind) {
        let _ = self.events.send(ExecutionEvent {
            run_id: self.id,
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
            path: self.path(),
            kind,
        });
    }
    pub fn operation(&self) -> &Operation {
        let frame = &self.frames[self.current()];
        frame.node_operation.as_ref().unwrap_or(&frame.operation)
    }
    pub fn check(&self) -> Result<(), RunError> {
        if !self.frames[self.current()].cleanup_mode {
            if self.root_operation.is_cancelled() {
                return Err(RunError::new(ErrorKind::Cancelled, "运行已取消"));
            }
            if self.root_operation.remaining().is_zero() {
                return Err(RunError::new(ErrorKind::RunTimeout, "运行总时限已到"));
            }
        }
        self.operation().check("workflow_node").map_err(Into::into)
    }
    pub fn complete_node(&mut self, native: Values) -> Result<(), RunError> {
        self.check()?;
        let current = self.current();
        let frame = &self.frames[current];
        let node = &frame.plan.scopes[frame.scope].nodes[frame.pc];
        let mapped = self.eval_fields(&node.mappings, &native)?;
        let mut published = native;
        published.extend(mapped);
        if published.keys().ne(node.output_types.keys())
            || published
                .iter()
                .any(|(name, value)| !value.matches(&node.output_types[name]))
        {
            return Err(RunError::new(
                ErrorKind::Contract,
                "节点输出违反冻结类型契约",
            ));
        }
        let size = published.values().fold(self.data_size(), |size, value| {
            size.saturating_add(value.measured_bytes().unwrap_or(usize::MAX))
        });
        if size > self.options.data_bytes {
            return Err(RunError::new(ErrorKind::Limit, "节点结果超过总数据预算"));
        }
        let pc = frame.pc;
        self.frames[current].outputs[pc] = Some(published);
        self.emit(EventKind::NodeCompleted);
        let frame = &mut self.frames[current];
        frame.pc += 1;
        frame.state = NodeState::Ready;
        frame.node_operation = None;
        frame.execution = None;
        Ok(())
    }
    pub fn cleanup_operation(&mut self) -> Result<Operation, RunError> {
        if let Some(operation) = &self.unwind_operation {
            return Ok(operation.clone());
        }
        let operation = Operation::new(
            OperationOptions::new(self.options.cleanup_timeout).map_err(RunError::from)?,
        );
        if self.root_operation.is_cancelled() || self.root_operation.remaining().is_zero() {
            self.unwind_operation = Some(operation.clone());
        }
        Ok(operation)
    }
    pub async fn leave(&mut self, mut signal: Signal) {
        if self.frames.len() == 1 || matches!(&signal, Signal::Error(error) if !error.catchable()) {
            let _ = self.status.send((RunStatus::Cleaning, None));
        }
        if let Signal::Error(error) = &mut signal {
            if error.path.is_empty() {
                error.path = self.path();
            }
            self.emit(EventKind::NodeFailed);
            if !error.catchable() && self.unwind_operation.is_none() {
                self.unwind_operation = self.cleanup_operation().ok();
            }
        }
        let owned = self.frames[self.current()].owned.clone();
        match self.cleanup_operation() {
            Ok(operation) => {
                for id in owned.into_iter().rev() {
                    if let Err(mut error) = self.pool.close(id, &operation).await {
                        error.path = self.path();
                        match &mut signal {
                            Signal::Error(primary) => primary.secondary.push(error),
                            _ => signal = Signal::Error(error),
                        }
                    }
                }
            }
            Err(error) => match &mut signal {
                Signal::Error(primary) => primary.secondary.push(error),
                _ => signal = Signal::Error(error),
            },
        }
        self.emit(EventKind::ScopeExited);
        self.frames.pop();
        self.incoming = Some(signal);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        scope: usize,
        lexical_parent: usize,
        inputs: Arc<Values>,
        resources: BTreeMap<String, Lease>,
        variables: Vec<(usize, Value)>,
        operation: Operation,
        cleanup_mode: bool,
    ) -> Result<(), RunError> {
        // 普通帧耗尽时保留独立的有限收尾栈，仍可执行已进入 Try 的 Finally。
        let depth_limit = self.options.max_depth + if cleanup_mode { 64 } else { 0 };
        if self.frames.len() >= depth_limit {
            return Err(RunError::new(ErrorKind::Limit, "活动调用帧超过预算"));
        }
        let parent = &self.frames[self.current()];
        let definition = &parent.plan.scopes[scope];
        let mut slots = vec![None; definition.slots.len()];
        for (slot, value) in variables {
            slots[slot] = Some(value);
        }
        let frame = Frame {
            plan: parent.plan.clone(),
            workflow_root: parent.workflow_root,
            scope,
            instance: self.next_instance,
            lexical_parent: Some(lexical_parent),
            inputs,
            variables: slots,
            outputs: vec![None; definition.nodes.len()],
            resources,
            owned: Vec::new(),
            pc: 0,
            execution: None,
            state: NodeState::Ready,
            operation,
            node_operation: None,
            cleanup_mode,
        };
        self.next_instance += 1;
        self.frames.push(frame);
        if self.data_size() > self.options.data_bytes {
            self.frames.pop();
            return Err(RunError::new(ErrorKind::Limit, "调用参数超过总数据预算"));
        }
        self.emit(EventKind::ScopeEntered);
        Ok(())
    }
}
