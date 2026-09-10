//! 自有应用启动、外部窗口附加与共享 UIA 来源绑定。
use crate::{AutomationHost, resources::*};
use argusflow_runtime::*;
use argusflow_windows::{Application, ApplicationOptions, WindowLocator};
use argusflow_workflow::{ErrorKind, Value, ValueType as Ty};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};

#[derive(Clone, Copy)]
pub(crate) enum DesktopKind {
    Launch,
    Window,
    Attach,
    Activate,
    Source,
}
impl DesktopKind {
    pub const ALL: [Self; 5] = [
        Self::Launch,
        Self::Window,
        Self::Attach,
        Self::Activate,
        Self::Source,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Launch => "application.launch",
            Self::Window => "application.wait_window",
            Self::Attach => "window.attach",
            Self::Activate => "window.activate",
            Self::Source => "source.uia",
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LaunchConfig {
    visible: bool,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct WindowConfig {
    title: Option<String>,
    class_name: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyConfig {}
pub(crate) fn compile(
    kind: DesktopKind,
    config: &serde_json::Value,
    host: Arc<AutomationHost>,
) -> Result<Arc<dyn PreparedTask>, String> {
    let mut visible = false;
    let mut window = WindowConfig::default();
    match kind {
        DesktopKind::Launch => {
            visible = serde_json::from_value::<LaunchConfig>(config.clone())
                .map_err(|e| e.to_string())?
                .visible;
        }
        DesktopKind::Window | DesktopKind::Attach => {
            window = serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
        }
        DesktopKind::Activate | DesktopKind::Source => {
            let _: EmptyConfig =
                serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
        }
    }
    if matches!(kind, DesktopKind::Source) && (host.uia.is_none() || host.input.is_none()) {
        return Err("UIA 来源要求宿主事先装配共享 UIA 和 Input 服务".into());
    }
    Ok(Arc::new(DesktopTask {
        kind,
        visible,
        window,
        host,
    }))
}
struct DesktopTask {
    kind: DesktopKind,
    visible: bool,
    window: WindowConfig,
    host: Arc<AutomationHost>,
}
impl PreparedTask for DesktopTask {
    fn signature(&self) -> TaskSignature {
        let mut s = TaskSignature::default();
        match self.kind {
            DesktopKind::Launch => {
                s.inputs = [
                    ("executable".into(), Ty::Text),
                    ("arguments".into(), Ty::List(Box::new(Ty::Text))),
                ]
                .into();
                s.resource_outputs
                    .insert("application".into(), APPLICATION.into());
            }
            DesktopKind::Window => {
                s.resources.insert("application".into(), APPLICATION.into());
                s.resource_outputs.insert("window".into(), WINDOW.into());
            }
            DesktopKind::Attach => {
                s.inputs.insert("process_id".into(), Ty::Int);
                s.resource_outputs.insert("window".into(), WINDOW.into());
            }
            DesktopKind::Activate => {
                s.resources.insert("window".into(), WINDOW.into());
            }
            DesktopKind::Source => {
                s.resources.insert("window".into(), WINDOW.into());
                s.resource_outputs.insert("source".into(), SOURCE.into());
            }
        }
        s
    }
    fn execute<'a>(&'a self, context: TaskContext<'a>) -> TaskFuture<'a, TaskOutput> {
        Box::pin(async move {
            let mut output = TaskOutput::default();
            match self.kind {
                DesktopKind::Launch => {
                    let Some(Value::List(arguments)) = context.inputs.get("arguments") else {
                        return Err(RunError::new(ErrorKind::Contract, "应用参数必须为文字列表"));
                    };
                    let arguments = arguments
                        .iter()
                        .map(|value| match value {
                            Value::Text(value) => Ok(value.clone()),
                            _ => Err(RunError::new(ErrorKind::Contract, "应用参数元素不是文字")),
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let mut options = ApplicationOptions::new(text(&context, "executable")?);
                    options.arguments = arguments;
                    options.visible = self.visible;
                    let application =
                        Application::launch(options, context.operation).map_err(native)?;
                    output.resources.insert(
                        "application".into(),
                        Arc::new(ApplicationResource(application)),
                    );
                }
                DesktopKind::Window => {
                    let application = resource::<ApplicationResource>(&context, "application")?;
                    let window = application
                        .0
                        .wait_window(
                            self.window.title.clone(),
                            self.window.class_name.clone(),
                            context.operation,
                        )
                        .await
                        .map_err(native)?;
                    output
                        .resources
                        .insert("window".into(), Arc::new(WindowResource(window)));
                }
                DesktopKind::Attach => {
                    let Some(Value::Int(pid)) = context.inputs.get("process_id") else {
                        return Err(RunError::new(ErrorKind::Contract, "process_id 必须为整数"));
                    };
                    let pid = u32::try_from(*pid)
                        .ok()
                        .filter(|pid| *pid > 0)
                        .ok_or_else(|| {
                            RunError::new(ErrorKind::Expression, "process_id 超出有效范围")
                        })?;
                    let locator = WindowLocator {
                        process_id: Some(pid),
                        title: self.window.title.clone(),
                        class_name: self.window.class_name.clone(),
                    };
                    loop {
                        context.operation.check("workflow_window_attach")?;
                        match locator.find_unique() {
                            Ok(window) => {
                                output.resources.insert(
                                    "window".into(),
                                    Arc::new(WindowResource(window.identity())),
                                );
                                break;
                            }
                            Err(error) if error.kind() == argusflow_core::FailureKind::NotFound => {
                                tokio::time::sleep(
                                    Duration::from_millis(20).min(context.operation.remaining()),
                                )
                                .await
                            }
                            Err(error) => return Err(native(error)),
                        }
                    }
                }
                DesktopKind::Activate => resource::<WindowResource>(&context, "window")?
                    .0
                    .activate(context.operation)
                    .map_err(native)?,
                DesktopKind::Source => {
                    let runtime =
                        self.host.uia.clone().ok_or_else(|| {
                            RunError::new(ErrorKind::Unavailable, "UIA 服务未装配")
                        })?;
                    let input =
                        self.host.input.clone().ok_or_else(|| {
                            RunError::new(ErrorKind::Unavailable, "Input 服务未装配")
                        })?;
                    let source = argusflow_automation::QuerySource::Uia {
                        runtime,
                        input,
                        window: resource::<WindowResource>(&context, "window")?.0.clone(),
                    };
                    output
                        .resources
                        .insert("source".into(), Arc::new(QuerySourceResource::new(source)));
                }
            }
            Ok(output)
        })
    }
}
fn native(error: argusflow_windows::WindowsError) -> RunError {
    argusflow_core::Failure::from(error).into()
}
