//! 独立调用复用同一运行预算、事件序列与资源池。
use super::{
    frame::{Frame, NodeState},
    runner::Runner,
    state::EventKind,
};
use crate::{PreparedWorkflow, RunError, resource::Lease};
use argusflow_workflow::{ErrorKind, Values};
use std::{collections::BTreeMap, sync::Arc};

impl Runner {
    pub(super) fn enter_workflow(
        &mut self,
        plan: Arc<PreparedWorkflow>,
        inputs: Arc<Values>,
        resources: BTreeMap<String, Lease>,
    ) -> Result<(), RunError> {
        let current = self.current();
        let cleanup_mode = self.frames[current].cleanup_mode;
        if self.frames.len() >= self.options.max_depth + if cleanup_mode { 64 } else { 0 } {
            return Err(RunError::new(ErrorKind::Limit, "活动调用帧超过预算"));
        }
        let definition = &plan.scopes[plan.root];
        let frame = Frame {
            scope: plan.root,
            workflow_root: self.frames.len(),
            instance: self.next_instance,
            lexical_parent: None,
            inputs,
            variables: vec![None; definition.slots.len()],
            outputs: vec![None; definition.nodes.len()],
            resources,
            owned: Vec::new(),
            pc: 0,
            execution: None,
            state: NodeState::Ready,
            operation: self.operation().clone(),
            node_operation: None,
            cleanup_mode,
            plan,
        };
        self.frames.push(frame);
        if self.data_size() > self.options.data_bytes {
            self.frames.pop();
            return Err(RunError::new(ErrorKind::Limit, "调用参数超过总数据预算"));
        }
        self.frames[current].state = NodeState::Call;
        self.next_instance += 1;
        self.emit(EventKind::ScopeEntered);
        Ok(())
    }
}
