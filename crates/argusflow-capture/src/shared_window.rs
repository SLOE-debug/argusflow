//! 按窗口与像素策略复用持续取帧任务，各消费者拥有独立游标。
use crate::{
    CaptureError, CaptureHealth, CapturePolicy, CapturedFrame, FrameId, FrameSubscription,
    PhysicalRect, TopologyGeneration, WindowFrameSource,
};
use argusflow_core::{WindowIdentity, capture::changes::FrameChange};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::sync::{Mutex, watch};

type FrameResult = Option<Result<Arc<CapturedFrame>, CaptureError>>;

/// 应用级共享窗口捕获门面；相同目标只打开一个原生订阅。
#[derive(Debug)]
pub struct SharedWindowSource {
    source: Arc<dyn WindowFrameSource>,
    sessions: Mutex<HashMap<(WindowIdentity, CapturePolicy), Weak<SharedSession>>>,
}

#[derive(Debug)]
struct SharedSession {
    native: Arc<dyn FrameSubscription>,
    frames: watch::Receiver<FrameResult>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for SharedSession {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl SharedWindowSource {
    /// 包装平台帧源，任务仅在第一个订阅到来时创建。
    pub fn new(source: Arc<dyn WindowFrameSource>) -> Self {
        Self {
            source,
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl WindowFrameSource for SharedWindowSource {
    fn health(&self) -> CaptureHealth {
        self.source.health()
    }
    async fn open(
        &self,
        window: WindowIdentity,
        policy: CapturePolicy,
    ) -> Result<Arc<dyn FrameSubscription>, CaptureError> {
        // 只串行化建立会话，不在正常取帧和消费者等待期间持有注册表锁。
        let mut sessions = self.sessions.lock().await;
        sessions.retain(|_, session| session.strong_count() > 0);
        let session = if let Some(session) = sessions.get(&(window, policy)).and_then(Weak::upgrade)
        {
            session
        } else {
            let native = self.source.open(window, policy).await?;
            let (sender, frames) = watch::channel(None);
            let task = tokio::spawn(publish(native.clone(), sender));
            let session = Arc::new(SharedSession {
                native,
                frames,
                task,
            });
            sessions.insert((window, policy), Arc::downgrade(&session));
            session
        };
        Ok(Arc::new(SharedSubscription {
            receiver: Mutex::new(session.frames.clone()),
            session,
            cursor: Mutex::new(None),
        }))
    }
}

#[derive(Debug)]
struct SharedSubscription {
    session: Arc<SharedSession>,
    receiver: Mutex<watch::Receiver<FrameResult>>,
    cursor: Mutex<Option<FrameId>>,
}

#[async_trait]
impl FrameSubscription for SharedSubscription {
    fn latest(&self) -> Result<Option<Arc<CapturedFrame>>, CaptureError> {
        self.session.frames.borrow().clone().transpose()
    }
    async fn next(&self, timeout: Duration) -> Result<Arc<CapturedFrame>, CaptureError> {
        let operation = async {
            let mut receiver = self.receiver.lock().await;
            let mut cursor = self.cursor.lock().await;
            loop {
                let current = receiver.borrow_and_update().clone();
                if let Some(result) = current {
                    let frame = result?;
                    if *cursor != Some(frame.frame_id) {
                        *cursor = Some(frame.frame_id);
                        return Ok(frame);
                    }
                }
                receiver
                    .changed()
                    .await
                    .map_err(|_| CaptureError::CaptureUnavailable {
                        message: "shared window capture stopped".into(),
                    })?;
            }
        };
        tokio::time::timeout(timeout, operation)
            .await
            .map_err(|_| CaptureError::FrameTimeout {
                timeout_ms: timeout.as_millis().min(u64::MAX as u128) as u64,
            })?
    }
    async fn current_topology_generation(&self) -> Result<TopologyGeneration, CaptureError> {
        self.session.native.current_topology_generation().await
    }
    fn window(&self) -> WindowIdentity {
        self.session.native.window()
    }
}

async fn publish(native: Arc<dyn FrameSubscription>, sender: watch::Sender<FrameResult>) {
    let mut previous: Option<Arc<CapturedFrame>> = None;
    let mut history = std::collections::VecDeque::new();
    loop {
        let frame = match native.next(Duration::from_secs(1)).await {
            Ok(frame) => frame,
            Err(CaptureError::FrameTimeout { .. }) => {
                tokio::time::sleep(Duration::from_millis(1)).await;
                continue;
            }
            Err(error) => {
                sender.send_replace(Some(Err(error)));
                break;
            }
        };
        if let Some(old) = &previous {
            if old.topology_generation == frame.topology_generation
                && old.width == frame.width
                && old.height == frame.height
                && old.window == frame.window
            {
                match exact_regions(old, &frame) {
                    Ok(regions) => history.push_back(FrameChange {
                        after: old.frame_id,
                        through: frame.frame_id,
                        regions: regions.into(),
                    }),
                    Err(error) => {
                        sender.send_replace(Some(Err(error)));
                        break;
                    }
                }
            } else {
                history.clear();
            }
        }
        while history.len() > 256
            || history
                .iter()
                .map(|change| change.regions.len())
                .sum::<usize>()
                > 8192
        {
            history.pop_front();
        }
        let published = Arc::new(
            (*frame)
                .clone()
                .with_change_history(history.iter().cloned().collect::<Vec<_>>().into()),
        );
        previous = Some(frame);
        sender.send_replace(Some(Ok(published)));
        if sender.is_closed() {
            break;
        }
    }
}

/// 离线帧和原生持续源使用同一个精确比较入口。
pub fn exact_regions(
    previous: &CapturedFrame,
    current: &CapturedFrame,
) -> Result<Vec<PhysicalRect>, CaptureError> {
    fn view(
        frame: &CapturedFrame,
    ) -> Result<crate::PixelView<'_>, argusflow_core::InspectionFailure> {
        crate::PixelView::new(
            frame.pixels(),
            frame.width,
            frame.height,
            frame.stride_bytes,
            argusflow_core::EvidencePixelFormat::Rgba8,
        )
    }
    // 视图借用各自的冻结像素，函数调用间不复制整张图。
    let old = view(previous).map_err(|_| invalid_layout())?;
    let new = view(current).map_err(|_| invalid_layout())?;
    crate::compare(old, new, None)
        .map(|changes| {
            changes
                .regions()
                .iter()
                .map(|rect| PhysicalRect {
                    x: rect.x as i32,
                    y: rect.y as i32,
                    width: rect.width,
                    height: rect.height,
                })
                .collect()
        })
        .map_err(|_| invalid_layout())
}

fn invalid_layout() -> CaptureError {
    CaptureError::InvalidFrame {
        message: "incomparable pixel layouts".into(),
    }
}
