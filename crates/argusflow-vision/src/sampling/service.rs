//! 取图到推理的总时限由一个任务拥有，不因阶段切换或重试而重置。
use super::{
    cache::{Cache, Cached, Entry, Flight, Key, Waiter},
    model::{SampledOcrError, SampledOcrResult},
};
use crate::{ImageInput, OcrEngine, OcrResult};
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation, OperationOptions};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::watch;

pub(super) struct Inner {
    pub(super) source: Arc<dyn RegionSource>,
    engine: OcrEngine,
    cache: Mutex<Cache>,
}
/// 固定 OCR 引擎和采样来源的区域服务；每个实例缓存最多八个完整区域。
#[derive(Clone)]
pub struct SampledOcr {
    pub(super) inner: Arc<Inner>,
}
impl SampledOcr {
    /// 使用现有模型实例，既不下载也不重复加载模型。
    pub fn new(source: Arc<dyn RegionSource>, engine: OcrEngine) -> Self {
        Self {
            inner: Arc::new(Inner {
                source,
                engine,
                cache: Mutex::new(Cache {
                    entries: HashMap::new(),
                    tick: 0,
                    capacity: 8,
                }),
            }),
        }
    }
    /// 识别完整区域；同键在途任务合并，缓存只在像素被精确确认相同时复用。
    pub async fn recognize(
        &self,
        source: SourceId,
        region: PixelRect,
        options: OperationOptions,
    ) -> Result<SampledOcrResult, SampledOcrError> {
        let caller = Operation::new(options);
        let key = Key { source, region };
        let (flight, start, previous) = {
            let mut cache = self.inner.cache.lock().unwrap_or_else(|p| p.into_inner());
            cache.tick += 1;
            let tick = cache.tick;
            if !cache.reserve(key) {
                return Err(busy());
            }
            let entry = cache.entries.entry(key).or_insert(Entry {
                cached: None,
                flight: None,
                used: tick,
            });
            entry.used = tick;
            if let Some(flight) = &entry.flight {
                flight.users.fetch_add(1, Ordering::AcqRel);
                (flight.clone(), false, None)
            } else {
                let (result, _) = watch::channel(None);
                let flight = Arc::new(Flight {
                    operation: Operation::new(options),
                    users: AtomicUsize::new(1),
                    result,
                });
                entry.flight = Some(flight.clone());
                (flight, true, entry.cached.clone())
            }
        };
        let _waiter = Waiter(flight.clone());
        if start {
            let inner = self.inner.clone();
            let job = flight.clone();
            tokio::spawn(async move {
                let source = inner.source.clone();
                let engine = inner.engine.clone();
                let operation = job.operation.clone();
                // 独立 JoinHandle 捕获消费者来源或推理 Future 的 panic，保证清理在途槽位。
                let result = tokio::spawn(async move {
                    run(
                        source,
                        key,
                        previous,
                        operation,
                        |input, operation| async move {
                            engine
                                .recognize_with_options(
                                    input,
                                    OperationOptions::new(operation.remaining())
                                        .map_err(crate::OcrError::from)?,
                                )
                                .await
                        },
                    )
                    .await
                })
                .await
                .unwrap_or_else(|_| {
                    Err(CaptureError::new(
                        FailureKind::Native,
                        "sampled_ocr_task",
                        "区域 OCR 任务异常退出",
                    )
                    .into())
                });
                let response = result
                    .as_ref()
                    .map(|cached| cached.output.clone())
                    .map_err(Clone::clone);
                {
                    let mut cache = inner.cache.lock().unwrap_or_else(|p| p.into_inner());
                    if let Some(entry) = cache.entries.get_mut(&key)
                        && entry
                            .flight
                            .as_ref()
                            .is_some_and(|active| Arc::ptr_eq(active, &job))
                    {
                        // 只有当前在途任务可写回；旧结果从不覆盖新的任务或版本。
                        if !job.operation.is_cancelled()
                            && let Ok(cached) = result
                        {
                            entry.cached = Some(cached);
                        }
                        entry.flight = None;
                    }
                }
                job.result.send_replace(Some(response));
            });
        }
        let mut receiver = flight.result.subscribe();
        loop {
            if let Some(result) = receiver.borrow_and_update().clone() {
                return result;
            }
            tokio::time::timeout(caller.remaining(), receiver.changed())
                .await
                .map_err(|_| {
                    SampledOcrError::Capture(CaptureError::new(
                        FailureKind::Timeout,
                        "sampled_ocr",
                        "区域 OCR 总时限已到",
                    ))
                })?
                .map_err(|_| {
                    SampledOcrError::Capture(CaptureError::new(
                        FailureKind::Unavailable,
                        "sampled_ocr",
                        "区域 OCR 任务已退出",
                    ))
                })?;
        }
    }
    /// 清除未执行的缓存，释放其 GPU 版本租约；在途任务不受影响。
    pub fn clear_cache(&self) {
        for entry in self
            .inner
            .cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entries
            .values_mut()
        {
            entry.cached = None;
        }
    }
}
fn busy() -> SampledOcrError {
    CaptureError::new(
        FailureKind::Busy,
        "sampled_ocr",
        "区域 OCR 缓存的在途额度已满",
    )
    .into()
}

pub(super) async fn run<F, Fut>(
    source: Arc<dyn RegionSource>,
    key: Key,
    previous: Option<Cached>,
    operation: Operation,
    recognize: F,
) -> Result<Cached, SampledOcrError>
where
    F: FnOnce(ImageInput, Operation) -> Fut,
    Fut: std::future::Future<Output = Result<OcrResult, crate::OcrError>>,
{
    let sampling = source.sample(
        SampleRequest {
            source: key.source,
            region: key.region,
            quiet: Duration::from_millis(150),
            previous: previous.as_ref().map(|cached| cached.token.clone()),
        },
        operation.clone(),
    );
    tokio::pin!(sampling);
    let sample = loop {
        tokio::select! {
            result=&mut sampling=>break result?,
            _=tokio::time::sleep(operation.remaining().min(Duration::from_millis(8)))=>operation.check("sampled_ocr_sampling").map_err(CaptureError::from)?,
        }
    };
    let bounds = sample.token.snapshot.bounds.project(key.region)?;
    let (result, reused) = match sample.content {
        SampleContent::Unchanged => {
            let cached = previous.ok_or_else(|| {
                CaptureError::new(
                    FailureKind::Protocol,
                    "sampled_ocr",
                    "没有先前内容却返回未变化",
                )
            })?;
            (cached.output.result, true)
        }
        SampleContent::Image(image) => {
            let inference = recognize(ImageInput::Shared(image), operation.clone());
            tokio::pin!(inference);
            let result = loop {
                tokio::select! {
                    result=&mut inference=>break result?,
                    _=tokio::time::sleep(operation.remaining().min(Duration::from_millis(8)))=>operation.check("sampled_ocr_inference").map_err(CaptureError::from)?,
                }
            };
            (Arc::new(result), false)
        }
    };
    operation
        .check("sampled_ocr_complete")
        .map_err(CaptureError::from)?;
    if !sample.token.snapshot.pixels.valid() {
        return Err(CaptureError::new(
            FailureKind::StaleHandle,
            "sampled_ocr_complete",
            "识别期间来源已重建",
        )
        .into());
    }
    Ok(Cached {
        token: sample.token.clone(),
        output: SampledOcrResult {
            token: sample.token,
            result,
            version: sample.observed_version,
            bounds,
            through: sample.observed_through,
            reused,
        },
    })
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-vision/unit/sampling/service.rs"]
mod tests;
