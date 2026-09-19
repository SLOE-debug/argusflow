//! 复用项目 DXGI 区域采样，保留窗口内可见浮层，不截取窗口以外内容。
use super::{
    Result,
    wechat_support::{desktop::Desktop, frame::Frame},
};
use argusflow_capture::FrameSampler;
use argusflow_capture_contracts::{
    DesktopFrameSource, FrameConfig, PixelRect, RegionSource, SampleRequest, SourceState,
};
use argusflow_core::{Operation, OperationOptions};
use argusflow_windows::DxgiFrameSource;
use std::{sync::Arc, time::Duration};
pub struct Screen {
    backend: Arc<DxgiFrameSource>,
    sampler: FrameSampler,
}
impl Screen {
    pub fn start() -> Result<Self> {
        let backend = Arc::new(DxgiFrameSource::start(FrameConfig {
            width: 3840,
            height: 2160,
            fps: 5,
            history: 4,
            ..Default::default()
        })?);
        let sampler = FrameSampler::new(backend.clone())?;
        Ok(Self { backend, sampler })
    }
    pub async fn frame(&self, desktop: &Desktop) -> Result<Frame> {
        let bounds = desktop.bounds()?;
        let source = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(source) = self.sampler.sources()?.into_iter().find(|s| {
                    s.state == SourceState::Ready
                        && bounds.x() >= s.bounds.x() - 16
                        && bounds.y() >= s.bounds.y() - 16
                        && i64::from(bounds.x()) + i64::from(bounds.width())
                            <= i64::from(s.bounds.x()) + i64::from(s.bounds.width()) + 16
                        && i64::from(bounds.y()) + i64::from(bounds.height())
                            <= i64::from(s.bounds.y()) + i64::from(s.bounds.height()) + 16
                }) {
                    return Ok::<_, Box<dyn std::error::Error>>(source);
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .map_err(|_| {
            format!(
                "窗口区域不在可用屏幕内：{bounds:?}；{:?}",
                self.sampler.sources()
            )
        })??;
        // 系统阴影可能越过屏幕边缘8px；只读取屏内交集，屏外边框填黑。
        let left = bounds.x().max(source.bounds.x());
        let top = bounds.y().max(source.bounds.y());
        let right = (bounds.x() + bounds.width() as i32)
            .min(source.bounds.x() + source.bounds.width() as i32);
        let bottom = (bounds.y() + bounds.height() as i32)
            .min(source.bounds.y() + source.bounds.height() as i32);
        let region = PixelRect::new(
            (left - source.bounds.x()) as u32,
            (top - source.bounds.y()) as u32,
            (right - left) as u32,
            (bottom - top) as u32,
        )?;
        let sample = self
            .sampler
            .sample(
                SampleRequest {
                    source: source.id,
                    region,
                    quiet: Duration::ZERO,
                    previous: None,
                },
                Operation::new(OperationOptions::default()),
            )
            .await?;
        let image = &sample.token.image;
        let mut bytes = vec![0; bounds.width() as usize * bounds.height() as usize * 4];
        for (y, row) in image.bytes().chunks_exact(image.stride()).enumerate() {
            let offset = ((y + (top - bounds.y()) as usize) * bounds.width() as usize
                + (left - bounds.x()) as usize)
                * 4;
            bytes[offset..offset + region.width() as usize * 4]
                .copy_from_slice(&row[..region.width() as usize * 4]);
        }
        desktop.window.require_foreground()?;
        Frame::from_bgrx(bounds, bytes)
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.backend
            .shutdown(Operation::new(OperationOptions::default()))
            .await?;
        Ok(())
    }
}
