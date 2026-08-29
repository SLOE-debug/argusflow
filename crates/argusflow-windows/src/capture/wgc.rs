//! Windows.Graphics.Capture 的异步帧源代理。

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use argusflow_core::WindowIdentity;
use argusflow_vision::{
    CapturePolicy, CapturedFrame, FrameId, FrameSubscription, TopologyGeneration, VisionError,
    WindowFrameSource,
};
use async_trait::async_trait;
use tokio::sync::Notify;

use super::host::{CaptureHostClient, CaptureSubscriptionId};

/// 绑定应用级 [`super::WindowsCaptureHost`] 的窗口帧源。
#[derive(Debug, Clone)]
pub struct WindowsGraphicsCapture {
    /// 唯一捕获线程的命令入口。
    client: Arc<CaptureHostClient>,
}

impl WindowsGraphicsCapture {
    /// 由应用级主机创建帧源，避免业务层绕过主机生命周期。
    pub(super) fn from_client(client: Arc<CaptureHostClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl WindowFrameSource for WindowsGraphicsCapture {
    async fn open(
        &self,
        window: WindowIdentity,
        policy: CapturePolicy,
    ) -> Result<Arc<dyn FrameSubscription>, VisionError> {
        let opened = self.client.open(window, policy).await?;
        Ok(Arc::new(WindowFrameSubscription {
            window,
            subscription_id: opened.subscription_id,
            notify: opened.notify,
            client: self.client.clone(),
            next_frame_id: AtomicU64::new(0),
        }))
    }
}

/// 异步订阅代理；WGC、拓扑和 D3D11 状态始终留在应用级捕获线程。
#[derive(Debug)]
struct WindowFrameSubscription {
    /// 订阅创建时冻结的 HWND/PID。
    window: WindowIdentity,
    /// 主机内部的强类型订阅标识。
    subscription_id: CaptureSubscriptionId,
    /// FrameArrived 的异步唤醒器。
    notify: Arc<Notify>,
    /// 唯一捕获线程的命令入口。
    client: Arc<CaptureHostClient>,
    /// 当前订阅内单调分配的帧 ID。
    next_frame_id: AtomicU64,
}

#[async_trait]
impl FrameSubscription for WindowFrameSubscription {
    async fn next(&self, timeout: Duration) -> Result<Arc<CapturedFrame>, VisionError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| frame_timeout(timeout))?;
        loop {
            if Instant::now() >= deadline {
                return Err(frame_timeout(timeout));
            }
            let frame_id = FrameId::new(self.next_frame_id.fetch_add(1, Ordering::Relaxed) + 1);
            if let Some(captured) = self
                .client
                .poll(self.subscription_id, frame_id, deadline, timeout)
                .await?
            {
                return Ok(captured);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(frame_timeout(timeout));
            }
            tokio::time::timeout(remaining, self.notify.notified())
                .await
                .map_err(|_| frame_timeout(timeout))?;
        }
    }

    async fn current_topology_generation(&self) -> Result<TopologyGeneration, VisionError> {
        self.client
            .current_topology_generation(self.subscription_id)
            .await
    }

    fn window(&self) -> WindowIdentity {
        self.window
    }
}

impl Drop for WindowFrameSubscription {
    fn drop(&mut self) {
        self.client.close(self.subscription_id);
    }
}

/// 把等待时限转换为视觉层统一的超时错误。
fn frame_timeout(timeout: Duration) -> VisionError {
    VisionError::FrameTimeout {
        timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
    }
}
