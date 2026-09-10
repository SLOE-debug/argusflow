//! 同一 Application 的克隆共享进程所有权；不绑定外部同名应用。
use super::native::NativeProcess;
use crate::{WindowIdentity, WindowLocator, WindowsError};
use argusflow_core::{FailureKind, Operation};
use std::{
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

static APPLICATION_SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
/// 独立 EXE 启动参数；不会解释 shell 命令或附加到单实例外部进程。
#[derive(Debug, Clone)]
pub struct ApplicationOptions {
    /// 绝对 EXE 路径。
    pub executable: PathBuf,
    /// 分离的参数，不包含可执行文件本身。
    pub arguments: Vec<String>,
    /// 是否显示窗口；后台验收进程应显式设置为 false。
    pub visible: bool,
}
impl ApplicationOptions {
    /// 默认创建可见应用。
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            arguments: Vec::new(),
            visible: true,
        }
    }
}
struct Inner {
    process: NativeProcess,
    stopping: AtomicBool,
    closed: AtomicBool,
    _permit: OwnedSemaphorePermit,
    cleanup: tokio::sync::Mutex<()>,
}
/// 受 Job 管理的自有应用进程树。
#[derive(Clone)]
pub struct Application {
    inner: Arc<Inner>,
}
impl Application {
    /// 挂起创建进程、绑定 Job 后恢复；任何失败都清理本次创建的资源。
    pub fn launch(
        options: ApplicationOptions,
        operation: &Operation,
    ) -> Result<Self, WindowsError> {
        if !options.executable.is_absolute()
            || !options.executable.is_file()
            || !options
                .executable
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
            || options.arguments.len() > 256
        {
            return Err(WindowsError::new(
                FailureKind::InvalidInput,
                "application_options",
                "需要现存的绝对 EXE 路径和不超过 256 个参数",
            ));
        }
        let permit = APPLICATION_SLOTS
            .get_or_init(|| Arc::new(Semaphore::new(8)))
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                WindowsError::new(
                    FailureKind::Busy,
                    "application_launch",
                    "应用进程树额度已满",
                )
            })?;
        let process = NativeProcess::launch(&options, operation)
            .map_err(|error| operation.contextualize(error))?;
        Ok(Self {
            inner: Arc::new(Inner {
                process,
                stopping: AtomicBool::new(false),
                closed: AtomicBool::new(false),
                _permit: permit,
                cleanup: tokio::sync::Mutex::new(()),
            }),
        })
    }
    /// 本次创建的根进程 ID，不能用于认领同名外部进程。
    pub fn process_id(&self) -> u32 {
        self.inner.process.pid
    }
    /// 是否已经确认整个 Job 的进程树退出。
    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::Acquire)
    }
    /// 等待本次根进程拥有的唯一可见窗口；不匹配单实例转交后的外部进程。
    pub async fn wait_window(
        &self,
        title: Option<String>,
        class_name: Option<String>,
        operation: &Operation,
    ) -> Result<WindowIdentity, WindowsError> {
        loop {
            operation.check("application_window")?;
            if self.inner.stopping.load(Ordering::Acquire)
                || self.inner.process.active_processes()? == 0
            {
                return Err(WindowsError::new(
                    FailureKind::Closed,
                    "application_window",
                    "自有应用已经退出或正在关闭",
                ));
            }
            match (WindowLocator {
                process_id: Some(self.process_id()),
                title: title.clone(),
                class_name: class_name.clone(),
            })
            .find_unique()
            {
                Ok(window) => return Ok(window.identity()),
                Err(error) if error.kind() == FailureKind::NotFound => {}
                Err(error) => return Err(error),
            }
            tokio::time::sleep(Duration::from_millis(20).min(operation.remaining())).await;
        }
    }
    /// 请求终止自有进程树并确认计数与句柄退出；超时仍保留对象。
    pub async fn shutdown(&self, operation: &Operation) -> Result<(), WindowsError> {
        let _cleanup = tokio::time::timeout(operation.remaining(), self.inner.cleanup.lock())
            .await
            .map_err(|_| {
                WindowsError::new(
                    FailureKind::Timeout,
                    "application_shutdown",
                    "等待应用关闭锁超时",
                )
            })?;
        if self.is_closed() {
            return Ok(());
        }
        operation.begin_effect("application_shutdown")?;
        if !self.inner.stopping.swap(true, Ordering::AcqRel)
            && let Err(error) = self.inner.process.terminate()
        {
            self.inner.stopping.store(false, Ordering::Release);
            return Err(error);
        }
        loop {
            if self.inner.process.exited()? {
                self.inner.closed.store(true, Ordering::Release);
                return Ok(());
            }
            operation.check("application_shutdown")?;
            tokio::time::sleep(Duration::from_millis(10).min(operation.remaining())).await;
        }
    }
}
