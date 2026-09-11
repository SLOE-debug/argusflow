//! 运行数据的词法激活帧；调用父级与词法父级分别建模。
use crate::{RunError, compilation::Binding, expression::ValueReader, resource::Lease};
use argusflow_core::Operation;
use argusflow_workflow::{Value, Values};
use std::{collections::BTreeMap, sync::Arc};

pub(super) enum Signal {
    Complete(Values),
    Return(Values),
    Break,
    Continue,
    Error(RunError),
}
pub(super) enum NodeState {
    Ready,
    Child,
    Call,
    Loop {
        iterations: u32,
        items: Option<Vec<Value>>,
    },
    Try {
        stage: TryStage,
        pending: Option<Signal>,
    },
}
#[derive(Clone, Copy)]
pub(super) enum TryStage {
    Body,
    Catch,
    Finally,
}
pub(super) struct Frame {
    pub plan: Arc<crate::PreparedWorkflow>,
    pub workflow_root: usize,
    pub scope: usize,
    pub instance: u64,
    pub lexical_parent: Option<usize>,
    pub inputs: Arc<Values>,
    pub variables: Vec<Option<Value>>,
    pub outputs: Vec<Option<Values>>,
    pub resources: BTreeMap<String, Lease>,
    pub owned: Vec<u64>,
    pub pc: usize,
    pub execution: Option<u64>,
    pub state: NodeState,
    pub operation: Operation,
    pub node_operation: Option<Operation>,
    pub cleanup_mode: bool,
}
pub(super) struct FrameReader<'a> {
    pub frames: &'a [Frame],
    pub current: usize,
}
impl FrameReader<'_> {
    pub fn frame(&self, scope: usize) -> Option<&Frame> {
        let mut current = Some(self.current);
        while let Some(index) = current {
            let frame = self.frames.get(index)?;
            if frame.scope == scope {
                return Some(frame);
            }
            current = frame.lexical_parent;
        }
        None
    }
}
impl ValueReader for FrameReader<'_> {
    fn variable(&self, binding: Binding) -> Option<&Value> {
        self.frame(binding.scope)?
            .variables
            .get(binding.slot)?
            .as_ref()
    }
    fn input(&self, name: &str) -> Option<&Value> {
        self.frames.get(self.current)?.inputs.get(name)
    }
    fn output(&self, scope: usize, node: usize, name: &str) -> Option<&Value> {
        self.frame(scope)?.outputs.get(node)?.as_ref()?.get(name)
    }
}
