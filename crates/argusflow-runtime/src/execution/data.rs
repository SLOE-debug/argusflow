//! 活跃帧的数据访问、空间计量和原子变量提交。
use super::{
    frame::{FrameReader, NodeState, Signal},
    runner::Runner,
};
use crate::{
    RunError,
    compilation::{Binding, ResourceBinding},
    expression::{Evaluation, PlanExpr},
    resource::Lease,
};
use argusflow_workflow::{ErrorKind, Value, Values};
use std::collections::BTreeMap;

impl Runner {
    pub fn eval(&self, expression: &PlanExpr, result: &Values) -> Result<Value, RunError> {
        let reader = FrameReader {
            frames: &self.frames,
            current: self.current(),
        };
        Evaluation {
            reader: &reader,
            result,
            operation: self.operation(),
            steps: 0,
            max_steps: self.options.expression_steps,
            max_bytes: self.options.value_bytes,
        }
        .eval(expression)
    }
    pub fn eval_fields(
        &self,
        fields: &BTreeMap<String, PlanExpr>,
        result: &Values,
    ) -> Result<Values, RunError> {
        let reader = FrameReader {
            frames: &self.frames,
            current: self.current(),
        };
        Evaluation {
            reader: &reader,
            result,
            operation: self.operation(),
            steps: 0,
            max_steps: self.options.expression_steps,
            max_bytes: self.options.value_bytes,
        }
        .fields(fields)
    }
    pub fn lexical_index(&self, scope: usize) -> Result<usize, RunError> {
        let mut current = Some(self.current());
        while let Some(index) = current {
            if self.frames[index].scope == scope {
                return Ok(index);
            }
            current = self.frames[index].lexical_parent;
        }
        Err(RunError::new(ErrorKind::Contract, "词法帧不存在"))
    }
    pub fn eval_assignments(
        &self,
        expressions: &[(Binding, PlanExpr)],
    ) -> Result<Vec<(Binding, Value)>, RunError> {
        let reader = FrameReader {
            frames: &self.frames,
            current: self.current(),
        };
        let result = Values::new();
        let mut evaluation = Evaluation {
            reader: &reader,
            result: &result,
            operation: self.operation(),
            steps: 0,
            max_steps: self.options.expression_steps,
            max_bytes: self.options.value_bytes,
        };
        let mut bytes = 0usize;
        let mut values = Vec::new();
        for (binding, expression) in expressions {
            let value = evaluation.eval(expression)?;
            bytes = bytes.saturating_add(value.measured_bytes().unwrap_or(usize::MAX));
            if bytes > self.options.data_bytes {
                return Err(RunError::new(ErrorKind::Limit, "赋值暂存数据超过预算"));
            }
            values.push((*binding, value));
        }
        Ok(values)
    }
    pub fn resources(
        &self,
        bindings: &BTreeMap<String, ResourceBinding>,
    ) -> Result<BTreeMap<String, Lease>, RunError> {
        bindings
            .iter()
            .map(|(port, binding)| {
                let index = self.lexical_index(binding.scope)?;
                let lease = self.frames[index]
                    .resources
                    .get(&binding.name)
                    .cloned()
                    .ok_or_else(|| RunError::new(ErrorKind::Stale, "资源已释放或尚未创建"))?;
                Ok((port.clone(), lease))
            })
            .collect()
    }
    pub fn data_size(&self) -> usize {
        self.frames.iter().fold(0usize, |total, frame| {
            let values = frame
                .variables
                .iter()
                .flatten()
                .chain(frame.outputs.iter().flatten().flat_map(|v| v.values()));
            let mut bytes = values.fold(0usize, |n, v| {
                n.saturating_add(v.measured_bytes().unwrap_or(usize::MAX))
            });
            bytes = bytes.saturating_add(frame.inputs.values().fold(0usize, |n, v| {
                n.saturating_add(v.measured_bytes().unwrap_or(usize::MAX))
            }));
            if let NodeState::Loop {
                items: Some(items), ..
            } = &frame.state
            {
                bytes = bytes.saturating_add(items.iter().fold(0usize, |n, v| {
                    n.saturating_add(v.measured_bytes().unwrap_or(usize::MAX))
                }));
            }
            if let NodeState::Try {
                pending: Some(Signal::Return(values) | Signal::Complete(values)),
                ..
            } = &frame.state
            {
                bytes = bytes.saturating_add(values.values().fold(0usize, |n, v| {
                    n.saturating_add(v.measured_bytes().unwrap_or(usize::MAX))
                }));
            }
            total.saturating_add(bytes)
        })
    }
    pub fn assign(
        &mut self,
        assignments: Vec<(Binding, Value)>,
        initialize: bool,
    ) -> Result<(), RunError> {
        let mut size = self.data_size();
        let mut writes = Vec::new();
        for (binding, value) in assignments {
            let index = self.lexical_index(binding.scope)?;
            let slot = self.frames[index]
                .variables
                .get(binding.slot)
                .ok_or_else(|| RunError::new(ErrorKind::Contract, "变量槽位不存在"))?;
            if initialize == slot.is_some() {
                return Err(RunError::new(
                    ErrorKind::Contract,
                    "重复初始化或赋值未初始化变量",
                ));
            }
            size = size
                .saturating_sub(slot.as_ref().and_then(Value::measured_bytes).unwrap_or(0))
                .saturating_add(value.measured_bytes().unwrap_or(usize::MAX));
            writes.push((index, binding.slot, value));
        }
        if size > self.options.data_bytes {
            return Err(RunError::new(ErrorKind::Limit, "变量事务超过总数据预算"));
        }
        for (index, slot, value) in writes {
            self.frames[index].variables[slot] = Some(value);
        }
        Ok(())
    }
}
