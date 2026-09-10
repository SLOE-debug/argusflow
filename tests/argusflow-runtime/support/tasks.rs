use argusflow_core::{Effect, Operation};
use argusflow_runtime::*;
use argusflow_workflow::*;
use std::{
    any::Any,
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

#[derive(Clone, Copy)]
pub enum Mode {
    ReadRetry,
    EffectFailure,
    EffectSuccess,
    Acquire,
    AcquireChild,
    InvalidAcquire,
    CompilePanic,
    Use,
    Hang,
    Panic,
}
pub struct Probe {
    pub attempts: AtomicUsize,
    pub cleaned: AtomicUsize,
    pub reject_cleanup: AtomicBool,
    pub cleanup_order: Mutex<Vec<usize>>,
    pub hold_cleanup: AtomicBool,
    pub cleanup_started: tokio::sync::Notify,
}
impl Default for Probe {
    fn default() -> Self {
        Self {
            attempts: AtomicUsize::new(0),
            cleaned: AtomicUsize::new(0),
            reject_cleanup: AtomicBool::new(false),
            cleanup_order: Mutex::new(Vec::new()),
            hold_cleanup: AtomicBool::new(false),
            cleanup_started: tokio::sync::Notify::new(),
        }
    }
}
pub struct Compiler {
    pub id: &'static str,
    pub mode: Mode,
    pub probe: Arc<Probe>,
}
impl NodeCompiler for Compiler {
    fn type_id(&self) -> &str {
        self.id
    }
    fn compile(
        &self,
        _version: u16,
        _config: &serde_json::Value,
    ) -> Result<Arc<dyn PreparedTask>, String> {
        if matches!(self.mode, Mode::CompilePanic) {
            panic!("deliberate compiler panic");
        }
        Ok(Arc::new(TaskProbe {
            mode: self.mode,
            probe: self.probe.clone(),
        }))
    }
}
struct TaskProbe {
    mode: Mode,
    probe: Arc<Probe>,
}
impl PreparedTask for TaskProbe {
    fn signature(&self) -> TaskSignature {
        let mut signature = TaskSignature::default();
        match self.mode {
            Mode::ReadRetry | Mode::EffectFailure => {
                signature.safe_to_retry = true;
            }
            Mode::EffectSuccess => {
                signature.outputs.insert("value".into(), ValueType::Int);
            }
            Mode::Acquire | Mode::AcquireChild | Mode::InvalidAcquire => {
                signature
                    .resource_outputs
                    .insert("out".into(), "test.resource".into());
                if matches!(self.mode, Mode::AcquireChild) {
                    signature
                        .resources
                        .insert("in".into(), "test.resource".into());
                }
            }
            Mode::Use => {
                signature
                    .resources
                    .insert("in".into(), "test.resource".into());
            }
            Mode::Hang | Mode::Panic | Mode::CompilePanic => {}
        }
        signature
    }
    fn execute<'a>(&'a self, context: TaskContext<'a>) -> TaskFuture<'a, TaskOutput> {
        Box::pin(async move {
            let attempt = self.probe.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            let mut output = TaskOutput::default();
            match self.mode {
                Mode::ReadRetry if attempt < 3 => {
                    return Err(RunError::new(ErrorKind::Busy, "temporary"));
                }
                Mode::EffectFailure => {
                    context.operation.begin_effect("test_effect")?;
                    return Err(RunError::new(ErrorKind::Busy, "uncertain")
                        .with_effect(Effect::Unconfirmed));
                }
                Mode::EffectSuccess => {
                    context.operation.begin_effect("test_effect")?;
                    output.values.insert("value".into(), Value::Int(1));
                }
                Mode::Acquire | Mode::AcquireChild | Mode::InvalidAcquire => {
                    output.resources.insert(
                        "out".into(),
                        Arc::new(TestResource {
                            probe: self.probe.clone(),
                            id: attempt,
                        }),
                    );
                    if matches!(self.mode, Mode::InvalidAcquire) {
                        output.values.insert("undeclared".into(), Value::Int(1));
                    }
                }
                Mode::Hang => std::future::pending::<()>().await,
                Mode::Panic => panic!("deliberate extension panic"),
                _ => {}
            }
            Ok(output)
        })
    }
}
pub struct TestResource {
    pub probe: Arc<Probe>,
    pub id: usize,
}
impl Resource for TestResource {
    fn resource_type(&self) -> &str {
        "test.resource"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn cleanup<'a>(&'a self, _operation: &'a Operation) -> TaskFuture<'a, ()> {
        Box::pin(async move {
            self.probe.cleanup_started.notify_one();
            if self.probe.hold_cleanup.load(Ordering::SeqCst) {
                std::future::pending::<()>().await;
            }
            if self.probe.reject_cleanup.load(Ordering::SeqCst) {
                return Err(RunError::new(ErrorKind::Cleanup, "still active"));
            }
            self.probe.cleaned.fetch_add(1, Ordering::SeqCst);
            self.probe.cleanup_order.lock().unwrap().push(self.id);
            Ok(())
        })
    }
}
pub fn task(id: &str, kind: &str) -> Node {
    Node::new(
        id,
        Action::Task {
            task: Task {
                type_id: kind.into(),
                version: 1,
                config: serde_json::json!({}),
                inputs: BTreeMap::new(),
                resources: BTreeMap::new(),
                resource_outputs: BTreeMap::new(),
                retry: None,
            },
        },
    )
}
pub fn configure(node: &mut Node) -> &mut Task {
    match &mut node.action {
        Action::Task { task } => task,
        _ => panic!("fixture"),
    }
}
pub fn retry(node: &mut Node) {
    configure(node).retry = Some(Retry {
        max_attempts: 3,
        initial_delay_ms: 1,
        max_delay_ms: 2,
        errors: vec![ErrorKind::Busy],
    });
}
pub fn registry(modes: Vec<(&'static str, Mode)>, probe: &Arc<Probe>) -> NodeRegistry {
    let mut registry = NodeRegistry::new();
    for (id, mode) in modes {
        registry
            .register(Arc::new(Compiler {
                id,
                mode,
                probe: probe.clone(),
            }))
            .unwrap();
    }
    registry
}
