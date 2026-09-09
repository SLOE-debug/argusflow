//! 请求完成前保留资源，超时之后仅协作清理已提交 GPU 工作。
use super::{compare::CompareJob, read::ReadJob};
use crate::capture::{
    gpu::{Graphics, Texture, TileMap},
    pixels::GpuPixels,
};
use argusflow_capture_contracts::*;
use argusflow_core::Operation;
use std::sync::Arc;
use tokio::sync::oneshot;

pub(crate) enum Request {
    Read {
        pixels: Arc<GpuPixels>,
        region: PixelRect,
        operation: Operation,
        reply: oneshot::Sender<CaptureResult<PixelImage>>,
    },
    Compare {
        pixels: Arc<GpuPixels>,
        before: Arc<TileMap>,
        regions: Vec<PixelRect>,
        operation: Operation,
        reply: oneshot::Sender<CaptureResult<PixelChanges>>,
    },
}
enum Job {
    Read(ReadJob),
    Compare(CompareJob),
}
pub(in crate::capture) struct Requests {
    active: Option<Job>,
    pub pool: Vec<Arc<Texture>>,
}
impl Requests {
    pub fn reject_waiting(receiver: &std::sync::mpsc::Receiver<Request>, limit: usize) {
        for _ in 0..limit {
            let Ok(request) = receiver.try_recv() else {
                break;
            };
            let error = CaptureError::new(
                argusflow_core::FailureKind::StaleHandle,
                "gpu_request",
                "来源正在重建",
            );
            match request {
                Request::Read { reply, .. } => {
                    let _ = reply.send(Err(error));
                }
                Request::Compare { reply, .. } => {
                    let _ = reply.send(Err(error));
                }
            }
        }
    }
    pub fn new() -> Self {
        Self {
            active: None,
            pool: Vec::new(),
        }
    }
    pub fn available(&self) -> bool {
        self.active.is_none()
    }
    pub fn start(&mut self, request: Request, graphics: &Graphics) {
        self.active = match request {
            Request::Read {
                pixels,
                region,
                operation,
                reply,
            } => {
                let reused = self.pool.pop();
                match ReadJob::new(pixels, region, operation, graphics, reused) {
                    Ok(mut job) => {
                        job.reply = Some(reply);
                        Some(Job::Read(job))
                    }
                    Err(error) => {
                        let _ = reply.send(Err(error));
                        None
                    }
                }
            }
            Request::Compare {
                pixels,
                before,
                regions,
                operation,
                reply,
            } => match CompareJob::new(pixels, before, regions, operation, graphics) {
                Ok(mut job) => {
                    job.reply = Some(reply);
                    Some(Job::Compare(job))
                }
                Err(error) => {
                    let _ = reply.send(Err(error));
                    None
                }
            },
        };
        // 一个提交批次只 Flush 一次；后续轮询不 Flush。
        unsafe { graphics.context.Flush() };
    }
    pub fn poll(
        &mut self,
        graphics: &Graphics,
        cpu: &ByteBudget,
        config: &BackendConfig,
        stats: &mut CaptureStats,
    ) -> CaptureResult<()> {
        let Some(job) = self.active.as_mut() else {
            return Ok(());
        };
        let done = match job {
            Job::Read(job) => job.poll(graphics, cpu, config, stats, &mut self.pool),
            Job::Compare(job) => job.poll(graphics, config, stats),
        }?;
        if done {
            self.active = None;
        }
        Ok(())
    }
}
