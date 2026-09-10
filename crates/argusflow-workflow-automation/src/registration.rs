//! 有限的内置适配节点清单；动态配置只在 compile 解码。
use crate::{AutomationHost, browser, query};
use argusflow_runtime::{NodeCompiler, NodeRegistry, PreparedTask};
use std::sync::Arc;

/// 将现有能力注册为任务；宿主在引擎运行前完成共享服务装配。
pub fn register_automation(
    registry: &mut NodeRegistry,
    host: AutomationHost,
) -> Result<(), String> {
    let host = Arc::new(host);
    for kind in query::QueryKind::ALL {
        registry.register(Arc::new(QueryCompiler {
            kind,
            host: host.clone(),
        }))?;
    }
    for kind in browser::BrowserKind::ALL {
        registry.register(Arc::new(BrowserCompiler { kind }))?;
    }
    #[cfg(windows)]
    for kind in crate::desktop::DesktopKind::ALL {
        registry.register(Arc::new(DesktopCompiler {
            kind,
            host: host.clone(),
        }))?;
    }
    Ok(())
}
struct QueryCompiler {
    kind: query::QueryKind,
    host: Arc<AutomationHost>,
}
impl NodeCompiler for QueryCompiler {
    fn type_id(&self) -> &str {
        self.kind.id()
    }
    fn compile(
        &self,
        version: u16,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn PreparedTask>, String> {
        version_one(version)?;
        query::compile(self.kind, config, self.host.clone())
    }
}
struct BrowserCompiler {
    kind: browser::BrowserKind,
}
impl NodeCompiler for BrowserCompiler {
    fn type_id(&self) -> &str {
        self.kind.id()
    }
    fn compile(
        &self,
        version: u16,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn PreparedTask>, String> {
        version_one(version)?;
        browser::compile(self.kind, config)
    }
}
#[cfg(windows)]
struct DesktopCompiler {
    kind: crate::desktop::DesktopKind,
    host: Arc<AutomationHost>,
}
#[cfg(windows)]
impl NodeCompiler for DesktopCompiler {
    fn type_id(&self) -> &str {
        self.kind.id()
    }
    fn compile(
        &self,
        version: u16,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn PreparedTask>, String> {
        version_one(version)?;
        crate::desktop::compile(self.kind, config, self.host.clone())
    }
}
fn version_one(version: u16) -> Result<(), String> {
    if version == 1 {
        Ok(())
    } else {
        Err("自动化任务只接受版本 1".into())
    }
}
