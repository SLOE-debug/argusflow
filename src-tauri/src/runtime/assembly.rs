//! 平台服务在桌面装配边界创建，节点仅借用共享能力。
use argusflow_core::OperationOptions;
use argusflow_runtime::NodeRegistry;
use argusflow_workflow_automation::{AutomationHost, register_automation};
use std::time::Duration;

/// 宿主持有共享服务直至全部运行完成。
pub struct Automation {
    /// 已冻结任务注册表。
    pub registry: NodeRegistry,
    host: AutomationHost,
}
impl Automation {
    /// 启动工作台所需 UIA 和输入服务，不操作现有用户窗口。
    pub async fn start() -> Result<Self, String> {
        let uia = argusflow_windows::UiaRuntime::start(Default::default(), options()?)
            .await
            .map_err(|e| e.to_string())?;
        let input = match argusflow_windows::InputService::new() {
            Ok(input) => input,
            Err(error) => {
                let _ = uia.shutdown(options()?).await;
                return Err(error.to_string());
            }
        };
        let host = AutomationHost {
            uia: Some(uia),
            input: Some(input),
            ..Default::default()
        };
        Self::from_host(host).await
    }
    /// 统一注册宿主能力；原生服务由调用方创建并转交生命周期。
    pub(super) async fn from_host(host: AutomationHost) -> Result<Self, String> {
        let mut registry = NodeRegistry::new();
        if let Err(error) = register_automation(&mut registry, host.clone()) {
            let service = Self { registry, host };
            let _ = service.shutdown().await;
            return Err(error);
        }
        Ok(Self { registry, host })
    }
    /// 关闭共享线程；调用方先取消并收尾所有工作流。
    pub async fn shutdown(&self) -> Result<(), String> {
        let mut errors = Vec::new();
        if let Some(input) = &self.host.input
            && let Err(error) = input.shutdown(options()?).await
        {
            errors.push(error.to_string());
        }
        if let Some(uia) = &self.host.uia
            && let Err(error) = uia.shutdown(options()?).await
        {
            errors.push(error.to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("；"))
        }
    }
}
fn options() -> Result<OperationOptions, String> {
    OperationOptions::new(Duration::from_secs(5)).map_err(|e| e.to_string())
}
