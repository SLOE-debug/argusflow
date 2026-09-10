//! 子作用域退出后的控制传播，以及不能覆盖首错的 Finally。
use super::{frame::*, runner::Runner};
use crate::{RunError, compilation::PlanAction};
use argusflow_workflow::{ErrorKind, Value, Values};
use std::collections::BTreeMap;

impl Runner {
    pub async fn resume(&mut self, signal: Signal) -> Result<(), RunError> {
        let current = self.current();
        let state = std::mem::replace(&mut self.frames[current].state, NodeState::Ready);
        match state {
            NodeState::Child => match signal {
                Signal::Complete(values) => self.complete_node(values),
                signal => {
                    self.leave(signal).await;
                    Ok(())
                }
            },
            NodeState::Call => match signal {
                Signal::Complete(values) | Signal::Return(values) => self.complete_node(values),
                Signal::Break | Signal::Continue => Err(RunError::new(
                    ErrorKind::Contract,
                    "循环跳转不能越过子流程边界",
                )),
                signal => {
                    self.leave(signal).await;
                    Ok(())
                }
            },
            NodeState::Loop { iterations, items } => match signal {
                Signal::Break => self.complete_node(Values::new()),
                Signal::Continue | Signal::Complete(_) => {
                    self.frames[current].state = NodeState::Loop { iterations, items };
                    Ok(())
                }
                signal => {
                    self.leave(signal).await;
                    Ok(())
                }
            },
            NodeState::Try { stage, pending } => self.resume_try(stage, pending, signal).await,
            NodeState::Ready => Err(RunError::new(
                ErrorKind::Contract,
                "无结构节点接收子作用域结果",
            )),
        }
    }
    pub(super) async fn resume_try(
        &mut self,
        stage: TryStage,
        pending: Option<Signal>,
        mut signal: Signal,
    ) -> Result<(), RunError> {
        let current = self.current();
        let plan = self.plan.clone();
        let frame = &self.frames[current];
        let PlanAction::Try {
            catches, finally, ..
        } = &plan.scopes[frame.scope].nodes[frame.pc].action
        else {
            return Err(RunError::new(ErrorKind::Contract, "Try 恢复节点不匹配"));
        };
        if matches!(stage, TryStage::Finally) {
            let mut primary = pending
                .ok_or_else(|| RunError::new(ErrorKind::Contract, "Finally 缺少待完成控制状态"))?;
            if let Signal::Error(error) = signal {
                match &mut primary {
                    Signal::Error(original) => original.secondary.push(error),
                    _ => primary = Signal::Error(error),
                }
            }
            return self.finish_try(primary).await;
        }
        // 父结构已取消或超时，不允许 Catch 在过期预算下启动新的业务路径。
        let can_resume = if let Err(mut error) = self.check() {
            error.path = self.path();
            match &mut signal {
                Signal::Error(primary) if !primary.catchable() => {}
                Signal::Error(primary) => primary.secondary.push(error),
                _ => signal = Signal::Error(error),
            }
            false
        } else {
            true
        };
        if can_resume
            && matches!(stage, TryStage::Body)
            && let Signal::Error(error) = &signal
            && error.catchable()
            && let Some(catch) = catches
                .iter()
                .find(|catch| catch.errors.contains(&error.kind))
        {
            let record = Value::Record(Values::from([
                ("kind".into(), Value::Text(format!("{:?}", error.kind))),
                (
                    "code".into(),
                    Value::Text(if error.kind == ErrorKind::User {
                        error.message.clone()
                    } else {
                        String::new()
                    }),
                ),
            ]));
            match self.enter_child(
                catch.scope,
                NodeState::Try {
                    stage: TryStage::Catch,
                    pending: None,
                },
                vec![(catch.error_slot, record)],
            ) {
                Ok(()) => return Ok(()),
                Err(mut failure) => {
                    failure.path = self.path();
                    if let Signal::Error(primary) = &mut signal {
                        primary.secondary.push(failure);
                    }
                }
            }
        }
        if let Some(scope) = finally {
            let operation = self.cleanup_operation()?;
            if matches!(&signal, Signal::Error(error) if !error.catchable())
                && self.unwind_operation.is_none()
            {
                self.unwind_operation = Some(operation.clone());
            }
            let inputs = self.frames[current].inputs.clone();
            self.frames[current].state = NodeState::Try {
                stage: TryStage::Finally,
                pending: Some(signal),
            };
            match self.push(
                *scope,
                current,
                inputs,
                BTreeMap::new(),
                Vec::new(),
                operation,
                true,
            ) {
                Ok(()) => return Ok(()),
                Err(mut failure) => {
                    failure.path = self.path();
                    let NodeState::Try {
                        pending: Some(pending),
                        ..
                    } = std::mem::replace(&mut self.frames[current].state, NodeState::Ready)
                    else {
                        return Err(RunError::new(
                            ErrorKind::Contract,
                            "Finally 压栈失败后丢失控制状态",
                        ));
                    };
                    signal = pending;
                    match &mut signal {
                        Signal::Error(primary) => primary.secondary.push(failure),
                        _ => signal = Signal::Error(failure),
                    }
                }
            }
        }
        self.finish_try(signal).await
    }
    async fn finish_try(&mut self, signal: Signal) -> Result<(), RunError> {
        match signal {
            Signal::Complete(values) => self.complete_node(values),
            signal => {
                self.leave(signal).await;
                Ok(())
            }
        }
    }
}
