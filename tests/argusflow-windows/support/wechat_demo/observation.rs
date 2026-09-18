//! 一次采集完整窗口；首次全图 OCR，后续只检测和识别变化小图。
use super::{desktop::Desktop, frame::Frame, incremental, snapshot::Observation, text};
use argusflow_capture_contracts::PixelRect;
use argusflow_core::{Operation, OperationOptions};
use argusflow_vision::{ImageInput, OcrEngine, PixelFormat};
use std::{
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// 带一份已成功提交的全窗口像素/文本基线，不共享跨窗口缓存。
pub struct Observer {
    engine: OcrEngine,
    previous: Option<Observation>,
}
impl Observer {
    /// 复用已加载的模型实例；初始无基线。
    pub fn new(engine: OcrEngine) -> Self {
        Self {
            engine,
            previous: None,
        }
    }
    /// 本轮所有区域识别完成才替换基线，任一区域失败都保留旧基线。
    pub async fn refresh(&mut self, desktop: &Desktop) -> Result<Observation> {
        let bounds = desktop.bounds()?;
        let pixels = super::capture::capture(desktop, bounds, bounds).await?;
        let frame = Frame::from_bgrx(bounds, pixels)?;
        self.recognize_frame(desktop, frame).await
    }
    /// 复用连续跟踪已采集的帧，避免为了 OCR 再截一次不同时间的画面。
    pub async fn recognize_frame(
        &mut self,
        desktop: &Desktop,
        frame: Frame,
    ) -> Result<Observation> {
        let started = Instant::now();
        let bounds = frame.bounds;
        if desktop.bounds()? != bounds {
            return Err("窗口几何改变，不能识别旧帧".into());
        }
        let operation = Operation::new(OperationOptions::new(Duration::from_secs(20))?);
        let previous = self.previous.as_ref();
        let old_blocks = previous.map(|p| p.blocks.as_slice()).unwrap_or_default();
        let plan = incremental::plan(previous.map(|p| &p.frame), &frame, old_blocks, &operation)?;
        let mut fresh = Vec::new();
        for region in &plan.regions {
            let result = self
                .engine
                .recognize_with_options(
                    ImageInput::Pixels {
                        width: region.width(),
                        height: region.height(),
                        stride: region.width() as usize * 3,
                        format: PixelFormat::Bgr,
                        bytes: frame.crop(*region)?,
                    },
                    OperationOptions::new(operation.remaining())?,
                )
                .await?;
            fresh.extend(text::project(&result, *region)?);
        }
        if desktop.bounds()? != bounds {
            return Err("识别期间窗口几何改变，未提交新基线".into());
        }
        let blocks = incremental::merge(old_blocks, &plan, fresh);
        let pixels: u64 = plan
            .regions
            .iter()
            .map(|r| u64::from(r.width()) * u64::from(r.height()))
            .sum();
        println!(
            "OCR {:?}: {} 个区域，输入像素 {}/{}，文本块 {}，耗时 {}ms",
            plan.kind,
            plan.regions.len(),
            pixels,
            u64::from(bounds.width()) * u64::from(bounds.height()),
            blocks.len(),
            started.elapsed().as_millis()
        );
        let observation = Observation {
            frame,
            blocks: Arc::new(blocks),
        };
        self.previous = Some(observation.clone());
        Ok(observation)
    }
    /// 只校验与动作相关的像素，其他聊天区动画不影响联系人点击。
    pub async fn confirm(
        &self,
        desktop: &Desktop,
        observation: &Observation,
        region: PixelRect,
    ) -> Result<()> {
        let bounds = observation.frame.bounds;
        if desktop.bounds()? != bounds {
            return Err("窗口已移动，旧坐标不可用".into());
        }
        let current = Frame::from_bgrx(
            bounds,
            super::capture::capture(desktop, bounds, bounds).await?,
        )?;
        if current.crop(region)? != observation.frame.crop(region)? {
            return Err("动作目标区域已变化，请重新观察".into());
        }
        Ok(())
    }
}
