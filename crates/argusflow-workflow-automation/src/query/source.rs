//! 在节点执行边界解析范围；查询节点不依赖额外的来源转换节点。
use super::config::TargetPlatform;
use crate::{AutomationHost, resources::*};
use argusflow_automation::QuerySource;
use argusflow_runtime::{RunError, TaskContext};
use argusflow_workflow::ErrorKind;

pub(super) fn resolve(
    platform: TargetPlatform,
    host: &AutomationHost,
    context: &TaskContext<'_>,
) -> Result<QuerySource, RunError> {
    match platform {
        TargetPlatform::Cdp => Ok(QuerySource::Browser(
            resource::<PageResource>(context, "scope")?.page.clone(),
        )),
        #[cfg(windows)]
        TargetPlatform::Uia => Ok(QuerySource::Uia {
            runtime: host
                .uia
                .clone()
                .ok_or_else(|| RunError::new(ErrorKind::Unavailable, "UIA 服务未装配"))?,
            input: host
                .input
                .clone()
                .ok_or_else(|| RunError::new(ErrorKind::Unavailable, "输入服务未装配"))?,
            window: resource::<WindowResource>(context, "scope")?.0.clone(),
        }),
        #[cfg(windows)]
        TargetPlatform::Ocr => {
            let source = resource::<QuerySourceResource>(context, "scope")?.resolve()?;
            if !matches!(source, QuerySource::Ocr { .. }) {
                return Err(RunError::new(
                    ErrorKind::Contract,
                    "OCR 范围必须绑定 OCR 来源，请检查节点平台与范围",
                ));
            }
            Ok(source)
        }
        #[cfg(not(windows))]
        TargetPlatform::Uia | TargetPlatform::Ocr => {
            let _ = host;
            Err(RunError::new(
                ErrorKind::Unavailable,
                "当前系统不支持此自动化平台",
            ))
        }
    }
}
