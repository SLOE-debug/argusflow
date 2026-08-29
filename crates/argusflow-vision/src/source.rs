//! 按窗口打开捕获订阅的跨平台接口与内存测试实现。

use std::{collections::VecDeque, fmt, sync::Arc, time::Duration};

use argusflow_core::WindowIdentity;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::frame::TopologyGeneration;
use crate::{error::VisionError, image::CapturedFrame};

/// HWND 捕获流的策略；像素和窗口作用域不在此处隐式扩大。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapturePolicy {
    /// FramePool 保留的缓冲数量。
    pub frame_pool_size: u32,
    /// 是否捕获系统光标。
    pub include_cursor: bool,
    /// 允许捕获实现为性能限制降低最大边长。
    pub max_dimension: Option<u32>,
}

impl Default for CapturePolicy {
    fn default() -> Self {
        Self {
            frame_pool_size: 3,
            include_cursor: false,
            max_dimension: None,
        }
    }
}

/// 由捕获后端提供的可复用窗口帧订阅。
#[async_trait]
pub trait FrameSubscription: fmt::Debug + Send + Sync {
    /// 在明确的超时时间内取得下一张帧。
    async fn next(&self, timeout: Duration) -> Result<Arc<CapturedFrame>, VisionError>;

    /// 读取当前窗口拓扑代数，不消费捕获帧。
    async fn current_topology_generation(&self) -> Result<TopologyGeneration, VisionError>;

    /// 返回订阅建立时冻结的窗口身份。
    fn window(&self) -> WindowIdentity;
}

/// 按 HWND 打开持续捕获流的抽象；具体 User32/WGC 代码留在 windows crate。
#[async_trait]
pub trait WindowFrameSource: fmt::Debug + Send + Sync {
    /// 为指定窗口打开一个新的共享捕获订阅。
    async fn open(
        &self,
        window: WindowIdentity,
        policy: CapturePolicy,
    ) -> Result<Arc<dyn FrameSubscription>, VisionError>;
}

/// 供 unit/golden test 和开发期注入使用的内存帧源。
#[derive(Debug, Default)]
pub struct MemoryFrameSource {
    /// 每个窗口对应的待消费帧队列。
    streams: std::sync::RwLock<Vec<(WindowIdentity, VecDeque<Arc<CapturedFrame>>)>>,
}

impl MemoryFrameSource {
    /// 创建空的内存帧源。
    pub fn new() -> Self {
        Self::default()
    }

    /// 为一个窗口安装按顺序消费的帧序列。
    pub fn insert(&self, window: WindowIdentity, frames: Vec<Arc<CapturedFrame>>) {
        let mut streams = self
            .streams
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let new_frames = frames.into_iter().collect::<VecDeque<_>>();
        if let Some((_, existing_frames)) = streams
            .iter_mut()
            .find(|(candidate, _)| *candidate == window)
        {
            *existing_frames = new_frames;
        } else {
            streams.push((window, new_frames));
        }
    }
}

#[derive(Debug)]
struct MemoryFrameSubscription {
    /// 订阅绑定的窗口身份。
    window: WindowIdentity,
    /// 该订阅独占的帧队列。
    frames: Mutex<VecDeque<Arc<CapturedFrame>>>,
    /// 最近一次已交付帧的拓扑代数，供输入前复验读取。
    topology_generation: Mutex<TopologyGeneration>,
}

#[async_trait]
impl WindowFrameSource for MemoryFrameSource {
    async fn open(
        &self,
        window: WindowIdentity,
        _policy: CapturePolicy,
    ) -> Result<Arc<dyn FrameSubscription>, VisionError> {
        let streams = self
            .streams
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let frames = streams
            .iter()
            .find(|(candidate, _)| *candidate == window)
            .map(|(_, frames)| frames.clone())
            .ok_or_else(|| VisionError::CaptureUnavailable {
                message: "memory frame stream is not registered".to_owned(),
            })?;
        let topology_generation = frames
            .front()
            .map(|frame| frame.topology_generation)
            .unwrap_or_else(|| TopologyGeneration::new(0));
        Ok(Arc::new(MemoryFrameSubscription {
            window,
            frames: Mutex::new(frames),
            topology_generation: Mutex::new(topology_generation),
        }))
    }
}

#[async_trait]
impl FrameSubscription for MemoryFrameSubscription {
    async fn next(&self, timeout: Duration) -> Result<Arc<CapturedFrame>, VisionError> {
        let frame = {
            let mut frames = self.frames.lock().await;
            frames.pop_front().ok_or(VisionError::FrameTimeout {
                timeout_ms: timeout.as_millis() as u64,
            })?
        };
        *self.topology_generation.lock().await = frame.topology_generation;
        Ok(frame)
    }

    async fn current_topology_generation(&self) -> Result<TopologyGeneration, VisionError> {
        Ok(*self.topology_generation.lock().await)
    }

    fn window(&self) -> WindowIdentity {
        self.window
    }
}
