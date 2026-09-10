//! 将固定浏览器操作转为任务端口与受控资源。
use crate::resources::*;
use argusflow_browser::{Browser, BrowserConfig, LaunchOptions};
use argusflow_runtime::*;
use argusflow_workflow::{Fields, Value, ValueType as Ty};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy)]
pub(crate) enum BrowserKind {
    Launch,
    Connect,
    Pages,
    Attach,
    NewPage,
    Navigate,
    Source,
}
impl BrowserKind {
    pub const ALL: [Self; 7] = [
        Self::Launch,
        Self::Connect,
        Self::Pages,
        Self::Attach,
        Self::NewPage,
        Self::Navigate,
        Self::Source,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Launch => "browser.launch",
            Self::Connect => "browser.connect",
            Self::Pages => "browser.pages",
            Self::Attach => "browser.attach",
            Self::NewPage => "browser.new_page",
            Self::Navigate => "browser.navigate",
            Self::Source => "source.dom",
        }
    }
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct LaunchConfig {
    #[serde(default)]
    headless: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyConfig {}
pub(crate) fn compile(
    kind: BrowserKind,
    config: &serde_json::Value,
) -> Result<Arc<dyn PreparedTask>, String> {
    let headless = if matches!(kind, BrowserKind::Launch) {
        serde_json::from_value::<LaunchConfig>(config.clone())
            .map_err(|e| e.to_string())?
            .headless
    } else {
        let _: EmptyConfig = serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
        false
    };
    Ok(Arc::new(BrowserTask { kind, headless }))
}
struct BrowserTask {
    kind: BrowserKind,
    headless: bool,
}
fn texts(names: &[&str]) -> Fields {
    names
        .iter()
        .map(|name| ((*name).into(), Ty::Text))
        .collect()
}
fn resource_port(name: &str, ty: &str) -> BTreeMap<String, String> {
    [(name.into(), ty.into())].into()
}
impl PreparedTask for BrowserTask {
    fn signature(&self) -> TaskSignature {
        let mut s = TaskSignature::default();
        match self.kind {
            BrowserKind::Launch => {
                s.inputs = texts(&["executable"]);
                s.resource_outputs = resource_port("browser", BROWSER);
            }
            BrowserKind::Connect => {
                s.inputs = texts(&["endpoint"]);
                s.resource_outputs = resource_port("browser", BROWSER);
            }
            BrowserKind::Pages => {
                s.resources = resource_port("browser", BROWSER);
                s.outputs.insert(
                    "pages".into(),
                    Ty::List(Box::new(Ty::Record(texts(&["id", "title", "url"])))),
                );
                s.safe_to_retry = true;
            }
            BrowserKind::Attach => {
                s.resources = resource_port("browser", BROWSER);
                s.inputs = texts(&["target_id"]);
                s.resource_outputs = resource_port("page", PAGE);
            }
            BrowserKind::NewPage => {
                s.resources = resource_port("browser", BROWSER);
                s.inputs = texts(&["url"]);
                s.resource_outputs = resource_port("page", PAGE);
            }
            BrowserKind::Navigate => {
                s.resources = resource_port("page", PAGE);
                s.inputs = texts(&["url"]);
            }
            BrowserKind::Source => {
                s.resources = resource_port("page", PAGE);
                s.resource_outputs = resource_port("source", SOURCE);
            }
        }
        s
    }
    fn execute<'a>(&'a self, context: TaskContext<'a>) -> TaskFuture<'a, TaskOutput> {
        Box::pin(async move {
            let mut output = TaskOutput::default();
            match self.kind {
                BrowserKind::Launch => {
                    let mut options = LaunchOptions::new(text(&context, "executable")?);
                    options.headless = self.headless;
                    let browser = Browser::launch_with_operation(
                        options,
                        BrowserConfig::default(),
                        context.operation,
                    )
                    .await
                    .map_err(native)?;
                    output
                        .resources
                        .insert("browser".into(), Arc::new(BrowserResource::new(browser)));
                }
                BrowserKind::Connect => {
                    let browser = Browser::connect_with_operation(
                        text(&context, "endpoint")?,
                        BrowserConfig::default(),
                        context.operation,
                    )
                    .await
                    .map_err(native)?;
                    output
                        .resources
                        .insert("browser".into(), Arc::new(BrowserResource::new(browser)));
                }
                BrowserKind::Pages => {
                    let pages = resource::<BrowserResource>(&context, "browser")?
                        .browser
                        .pages_with_operation(context.operation)
                        .await
                        .map_err(native)?;
                    output.values.insert(
                        "pages".into(),
                        Value::List(
                            pages
                                .iter()
                                .map(|page| {
                                    Value::Record(
                                        [
                                            ("id".into(), Value::Text(page.target_id().into())),
                                            ("title".into(), Value::Text(page.title().into())),
                                            ("url".into(), Value::Text(page.url().into())),
                                        ]
                                        .into(),
                                    )
                                })
                                .collect(),
                        ),
                    );
                }
                BrowserKind::Attach => {
                    let page = resource::<BrowserResource>(&context, "browser")?
                        .attach(text(&context, "target_id")?, context.operation)
                        .await?;
                    output.resources.insert("page".into(), Arc::new(page));
                }
                BrowserKind::NewPage => {
                    let page = resource::<BrowserResource>(&context, "browser")?
                        .new_page(text(&context, "url")?, context.operation)
                        .await?;
                    output.resources.insert("page".into(), Arc::new(page));
                }
                BrowserKind::Navigate => {
                    resource::<PageResource>(&context, "page")?
                        .page
                        .navigate_with_operation(text(&context, "url")?, context.operation)
                        .await
                        .map_err(native)?;
                }
                BrowserKind::Source => {
                    output.resources.insert(
                        "source".into(),
                        Arc::new(QuerySourceResource::new(
                            argusflow_automation::QuerySource::Browser(
                                resource::<PageResource>(&context, "page")?.page.clone(),
                            ),
                        )),
                    );
                }
            }
            Ok(output)
        })
    }
}
fn native(error: argusflow_browser::BrowserError) -> RunError {
    argusflow_core::Failure::from(error).into()
}
