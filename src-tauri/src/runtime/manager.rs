//! 运行句柄由宿主任务持有，前端重新挂载不会取消或重复启动。
use super::{assembly::Automation, journal::RunJournal, messages::*};
use argusflow_runtime::{EventRead, RunInputs, RunOptions, WorkflowEngine, prepare_bundle};
use argusflow_workflow::{Diagnostic, Values, WorkflowBundle};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::ipc::Channel;
use tokio::sync::{Mutex, OnceCell, mpsc};

#[cfg(test)]
#[path = "../../../tests/argusflow-desktop/unit/manager.rs"]
mod tests;

/// 单工作台运行服务，能力延迟初始化。
pub struct RunManager {
    automation: OnceCell<Automation>,
    engine: WorkflowEngine,
    active: Mutex<Option<mpsc::Sender<()>>>,
    journal: RunJournal,
    /// 关闭期间拒绝新的运行，失败后仍可重试关闭。
    closing: AtomicBool,
}
impl Default for RunManager {
    fn default() -> Self {
        Self {
            automation: OnceCell::new(),
            engine: WorkflowEngine::new(),
            active: Mutex::new(None),
            journal: RunJournal::default(),
            closing: AtomicBool::new(false),
        }
    }
}
impl RunManager {
    /// 当前宿主的已注册能力，不启动业务动作。
    pub async fn capabilities(&self) -> Result<Vec<String>, String> {
        let automation = self.automation.get_or_try_init(Automation::start).await?;
        Ok(automation.registry.type_ids().map(str::to_owned).collect())
    }
    /// 从真实节点编译器获取动态输入类型。
    pub async fn task_inputs(
        &self,
        type_id: String,
        mut config: serde_json::Value,
    ) -> Result<argusflow_workflow::Fields, String> {
        let automation = self.automation.get_or_try_init(Automation::start).await?;
        crate::document::wire::decode_config(&type_id, &mut config)?;
        let task = argusflow_workflow::Task {
            type_id,
            version: 1,
            config,
            inputs: BTreeMap::new(),
            resources: BTreeMap::new(),
            resource_outputs: BTreeMap::new(),
            retry: None,
        };
        automation
            .registry
            .describe(&task)
            .map(|signature| signature.inputs)
    }
    /// 静态校验不会执行工作流动作。
    pub async fn validate(&self, bundle: WorkflowBundle) -> Result<Vec<Diagnostic>, String> {
        let automation = self.automation.get_or_try_init(Automation::start).await?;
        Ok(prepare_bundle(bundle, &automation.registry)
            .err()
            .unwrap_or_default())
    }
    /// 开始一个冻结计划；活动句柄直到清理结束才释放。
    pub async fn start(
        self: &Arc<Self>,
        bundle: WorkflowBundle,
        inputs: Values,
        channel: Channel<RunMessage>,
    ) -> Result<String, String> {
        let mut active = self.active.lock().await;
        if self.closing.load(Ordering::Acquire) {
            return Err("正在关闭工作台，请等待收尾完成".into());
        }
        if active.is_some() {
            return Err("已有工作流正在运行或收尾".into());
        }
        let automation = self.automation.get_or_try_init(Automation::start).await?;
        let root = bundle.root.0.clone();
        let documents = bundle.workflows.keys().map(|id| id.0.clone()).collect();
        let plan = prepare_bundle(bundle, &automation.registry).map_err(|errors| {
            errors
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
                .join("；")
        })?;
        let mut handle = self
            .engine
            .start(
                plan,
                RunInputs {
                    values: inputs,
                    ..Default::default()
                },
                RunOptions::default(),
            )
            .map_err(|e| e.to_string())?;
        let id = handle.id().to_string();
        let mut events = handle.subscribe();
        let snapshot = RunSnapshot {
            id: id.clone(),
            workflow: root,
            documents,
            status: DesktopRunStatus::Running,
            logs: Vec::new(),
            omitted: 0,
            outputs: serde_json::json!({}),
            errors: Vec::new(),
        };
        self.journal.begin(snapshot, channel).await;
        let (sender, mut cancel) = mpsc::channel(1);
        *active = Some(sender);
        let manager = self.clone();
        tokio::spawn(async move {
            let started = Instant::now();
            let mut interval = tokio::time::interval(Duration::from_millis(30));
            let mut event_open = true;
            loop {
                tokio::select! {
                    Some(()) = cancel.recv() => handle.cancel(),
                    item = events.recv(), if event_open => match item {
                        EventRead::Event(value) => manager.journal.append(super::messages::event(value, started.elapsed().as_millis())).await,
                        EventRead::Gap(count) => {
                            manager.journal.append(LogEntry { sequence: format!("gap-{}", started.elapsed().as_nanos()), elapsed_ms: started.elapsed().as_millis().to_string(), kind: "gap".into(), level: "warning".into(), message: format!("事件消费落后，缺少 {count} 条记录"), path: Vec::new() }).await;
                        }
                        EventRead::Closed => event_open = false,
                    },
                    _ = interval.tick() => {
                        manager.journal.progress(handle.status()).await;
                        if !event_open && let Some(result) = handle.result() {
                            let mut active = manager.active.lock().await;
                            manager.journal.finish(&result, started.elapsed().as_millis()).await;
                            *active = None;
                            break;
                        }
                    }
                }
            }
        });
        Ok(id)
    }
    /// 返回当前或最近一次结果，不修改订阅。
    pub async fn snapshot(&self) -> Option<RunSnapshot> {
        self.journal.snapshot().await
    }
    /// 完整初始状态与后续事件使用同一 Channel，避免响应覆盖较新的消息。
    pub async fn subscribe(&self, channel: Channel<RunMessage>) {
        self.journal.subscribe(channel).await;
    }
    /// 请求协作停止，等待原引擎有界收尾。
    pub async fn cancel(&self) -> Result<(), String> {
        if let Some(sender) = self.active.lock().await.as_ref() {
            let _ = sender.try_send(());
        }
        Ok(())
    }
    /// 窗口关闭后保持运行时直到任务和共享服务收尾。
    pub async fn shutdown(&self) -> Result<(), String> {
        {
            let active = self.active.lock().await;
            if self.closing.swap(true, Ordering::AcqRel) {
                return Err("正在关闭工作台，请等待收尾完成".into());
            }
            if let Some(sender) = active.as_ref() {
                let _ = sender.try_send(());
            }
        }
        let result = self.finish_shutdown().await;
        if result.is_err() {
            self.closing.store(false, Ordering::Release);
        }
        result
    }
    async fn finish_shutdown(&self) -> Result<(), String> {
        // 不释放尚在执行的句柄。等待超时后保留窗口，让用户稍后重试关闭。
        tokio::time::timeout(Duration::from_secs(15), async {
            while self.active.lock().await.is_some() {
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
        })
        .await
        .map_err(|_| "工作流仍在收尾，窗口已保留。请稍后重试关闭。".to_owned())?;
        let mut errors = Vec::new();
        if let Err(error) = self.engine.retry_cleanup(Duration::from_secs(5)).await {
            errors.push(format!("{error:?}"));
        }
        if let Some(automation) = self.automation.get()
            && let Err(error) = automation.shutdown().await
        {
            errors.push(error);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("；"))
        }
    }
}
