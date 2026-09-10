//! 专用 MTA 线程拥有全部 COM 对象和元素租约。
use super::{action, element::Lease, query, runtime::Shared};
use crate::WindowsError as Failure;
use crate::{
    ElementHandle, ElementSnapshot, Query, UiaAction, UiaConfig, UiaState,
    platform::{Apartment, PhysicalDpi, failure},
};
use argusflow_core::{FailureKind, Operation};
use std::{
    collections::HashMap,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use windows::Win32::{
    System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
    UI::Accessibility::{CUIAutomation8, IUIAutomation2, IUIAutomationElement},
};

pub(crate) enum Command {
    Aql(crate::WindowIdentity, argusflow_aql::BoundQuery),
    ClickPoint(ElementHandle),
    FocusAql(ElementHandle),
    Find(Query, bool),
    Read(ElementHandle),
    Act(ElementHandle, UiaAction),
}
pub(crate) enum Response {
    Aql(Vec<super::aql::UiaMatch>),
    Point(argusflow_core::ScreenPoint),
    Elements(Vec<ElementHandle>),
    Snapshot(ElementSnapshot),
    Done,
}
pub(crate) struct Request {
    pub(crate) command: Command,
    pub(crate) operation: Operation,
    pub(crate) response: oneshot::Sender<Result<Response, Failure>>,
}
struct Cached {
    element: IUIAutomationElement,
    expires: Instant,
    lease: Weak<Lease>,
}
struct Provider {
    automation: IUIAutomation2,
    cache: HashMap<u64, Cached>,
    next_element: u64,
    id: u64,
    config: UiaConfig,
    _apartment: Apartment,
    _dpi: PhysicalDpi,
}

pub(crate) trait Backend {
    fn prune(&mut self);
    fn handle(&mut self, command: Command, operation: &Operation) -> Result<Response, Failure>;
}
impl Backend for Provider {
    fn prune(&mut self) {
        Provider::prune(self);
    }
    fn handle(&mut self, command: Command, operation: &Operation) -> Result<Response, Failure> {
        Provider::handle(self, command, operation)
    }
}

impl Provider {
    fn new(config: UiaConfig, id: u64) -> Result<Self, Failure> {
        let dpi = PhysicalDpi::enter()?;
        let apartment = Apartment::new()?;
        // SAFETY: 初始化在专用 MTA 线程，接口和 apartment 一起释放。
        let automation: IUIAutomation2 =
            unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }
                .map_err(|e| failure("uia_create", e))?;
        // SAFETY: timeout 由配置入口校验为非零 u32 毫秒。
        unsafe {
            automation
                .SetConnectionTimeout(config.connection_timeout.as_millis() as u32)
                .map_err(|e| failure("connection_timeout", e))?;
            automation
                .SetTransactionTimeout(config.transaction_timeout.as_millis() as u32)
                .map_err(|e| failure("transaction_timeout", e))?;
        }
        Ok(Self {
            automation,
            cache: HashMap::new(),
            next_element: 1,
            id,
            config,
            _apartment: apartment,
            _dpi: dpi,
        })
    }
    fn prune(&mut self) {
        self.cache.retain(|_, cached| {
            cached.expires > Instant::now()
                && cached
                    .lease
                    .upgrade()
                    .is_some_and(|lease| lease.alive.load(Ordering::Acquire))
        });
    }
    fn handle(&mut self, command: Command, operation: &Operation) -> Result<Response, Failure> {
        operation.check("uia_execute")?;
        self.prune();
        match command {
            Command::Aql(window, query) => {
                let found =
                    super::aql::find(&self.automation, &window, &query, operation, &self.config)?;
                let handles = self.lease(
                    found.iter().map(|entry| entry.element.clone()).collect(),
                    &window,
                )?;
                Ok(Response::Aql(
                    found
                        .into_iter()
                        .zip(handles)
                        .map(|(entry, handle)| {
                            super::aql::UiaMatch::new(handle, entry.snapshot, entry.attributes)
                        })
                        .collect(),
                ))
            }
            Command::ClickPoint(handle) => {
                let element = self.element(&handle, operation)?;
                Ok(Response::Point(super::aql::click_point(
                    &self.automation,
                    &element,
                    operation,
                )?))
            }
            Command::FocusAql(handle) => {
                let element = self.element(&handle, operation)?;
                super::aql::focus(&element, operation)?;
                Ok(Response::Done)
            }
            Command::Find(query, unique) => {
                let found = query::find(&self.automation, &query, operation, &self.config, unique)?;
                let handles = self.lease(found, &query.window)?;
                Ok(Response::Elements(handles))
            }
            Command::Read(handle) => {
                let element = self.element(&handle, operation)?;
                let result = query::snapshot(&element)?;
                operation.check("uia_read_complete")?;
                Ok(Response::Snapshot(result))
            }
            Command::Act(handle, action) => {
                let element = self.element(&handle, operation)?;
                action::execute(&element, action, operation)?;
                Ok(Response::Done)
            }
        }
    }
    fn element(
        &self,
        handle: &ElementHandle,
        operation: &Operation,
    ) -> Result<IUIAutomationElement, Failure> {
        if handle.runtime != self.id || !handle.lease.alive.load(Ordering::Acquire) {
            return Err(stale());
        }
        let cached = self.cache.get(&handle.id).ok_or_else(stale)?;
        if cached.expires <= Instant::now() {
            return Err(stale());
        }
        query::validate_membership(
            &self.automation,
            &cached.element,
            &handle.window,
            operation,
            self.config.max_depth,
        )?;
        Ok(cached.element.clone())
    }
    fn lease(
        &mut self,
        found: Vec<IUIAutomationElement>,
        window: &crate::WindowIdentity,
    ) -> Result<Vec<ElementHandle>, Failure> {
        if self.cache.len() + found.len() > self.config.max_results {
            return Err(Failure::new(
                FailureKind::ResourceLimit,
                "element_lease",
                "元素租约已满",
            ));
        }
        let mut handles = Vec::with_capacity(found.len());
        for element in found {
            let id = self.next_element;
            self.next_element = self.next_element.checked_add(1).ok_or_else(|| {
                Failure::new(FailureKind::ResourceLimit, "element_lease", "元素标识耗尽")
            })?;
            let lease = Arc::new(Lease {
                alive: AtomicBool::new(true),
            });
            self.cache.insert(
                id,
                Cached {
                    element,
                    expires: Instant::now() + self.config.lease_duration,
                    lease: Arc::downgrade(&lease),
                },
            );
            handles.push(ElementHandle {
                runtime: self.id,
                id,
                window: window.clone(),
                lease,
            });
        }
        Ok(handles)
    }
}

fn stale() -> Failure {
    Failure::new(
        FailureKind::StaleHandle,
        "element_lease",
        "元素租约已经失效",
    )
}

pub(crate) fn run(
    receiver: Receiver<Request>,
    ready: oneshot::Sender<Result<(), Failure>>,
    shared: Arc<Shared>,
    config: UiaConfig,
    id: u64,
) {
    run_backend(receiver, ready, shared, || Provider::new(config, id));
}

pub(crate) fn run_backend<B: Backend>(
    receiver: Receiver<Request>,
    ready: oneshot::Sender<Result<(), Failure>>,
    shared: Arc<Shared>,
    create: impl FnOnce() -> Result<B, Failure>,
) {
    // panic 仅在 worker 边界转换为明确失败，未跨过 FFI 展开。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut provider = match create() {
            Ok(provider) => provider,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };
        *shared.state.lock().unwrap_or_else(|p| p.into_inner()) = UiaState::Ready;
        if ready.send(Ok(())).is_err() {
            return;
        }
        loop {
            if shared.stopping.load(Ordering::Acquire) {
                break;
            }
            provider.prune();
            match receiver.recv_timeout(Duration::from_millis(25)) {
                Ok(request) => {
                    if shared.stopping.load(Ordering::Acquire) {
                        let _ = request.response.send(Err(Failure::new(
                            FailureKind::Closed,
                            "uia_execute",
                            "UIA 正在关闭",
                        )));
                        continue;
                    }
                    *shared.active.lock().unwrap_or_else(|p| p.into_inner()) =
                        Some(request.operation.clone());
                    let started = Instant::now();
                    let result = request
                        .operation
                        .check("uia_execute")
                        .map_err(Failure::from)
                        .and_then(|_| provider.handle(request.command, &request.operation))
                        .map_err(|error| request.operation.contextualize(error));
                    tracing::debug!(
                        request_id = request.operation.id(),
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        success = result.is_ok(),
                        "uia request completed"
                    );
                    // 迟到句柄在当前线程释放，绝不把已取消的结果交给调用者。
                    let result = request
                        .operation
                        .check("uia_response")
                        .map_err(Failure::from)
                        .and(result);
                    let _ = request.response.send(result);
                    *shared.active.lock().unwrap_or_else(|p| p.into_inner()) = None;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        *shared.state.lock().unwrap_or_else(|p| p.into_inner()) = UiaState::Stopping;
        // provider 的 COM 字段先于 apartment 销毁。
        drop(provider);
    }));
    *shared.active.lock().unwrap_or_else(|p| p.into_inner()) = None;
    *shared.state.lock().unwrap_or_else(|p| p.into_inner()) = if result.is_ok() {
        UiaState::Stopped
    } else {
        UiaState::Failed
    };
    shared.finished.notify_waiters();
}
