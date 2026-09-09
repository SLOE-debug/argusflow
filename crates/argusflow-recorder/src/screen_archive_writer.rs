//! 两个编码线程与有序提交器；容量和失败都不会阻塞采样。

use crate::screen_archive::*;
use argusflow_capture::{FrameUpdate, PixelRect};
use argusflow_core::{CaptureFailure, InspectionRect};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
};

struct ArchiveJob {
    frame: FrameUpdate,
    record: ScreenFrame,
    regions: Vec<PixelRect>,
    bytes: usize,
}

struct CommitState {
    next: u64,
    ready: BTreeMap<u64, Result<ScreenFrame, CaptureFailure>>,
    timeline: ScreenTimeline,
}

/// 归档结果供生命周期状态只读检查。
#[derive(Clone)]
pub(crate) struct ArchiveResult(Arc<Mutex<CommitState>>);

impl ArchiveResult {
    pub(crate) fn observe_capture(&self, bytes: usize, diff_us: u64) {
        if let Ok(mut state) = self.0.lock() {
            state.timeline.diagnostics.readback_bytes = state
                .timeline
                .diagnostics
                .readback_bytes
                .saturating_add(bytes as u64);
            state.timeline.diagnostics.diff_total_us = state
                .timeline
                .diagnostics
                .diff_total_us
                .saturating_add(diff_us);
        }
    }
    pub(crate) fn failed(&self) -> bool {
        self.0.lock().map_or(true, |state| {
            matches!(
                state.timeline.completeness,
                ScreenCompleteness::Incomplete { .. }
            )
        })
    }
    pub(crate) fn fail(&self, reason: CaptureFailure) {
        if let Ok(mut state) = self.0.lock() {
            if !matches!(
                state.timeline.completeness,
                ScreenCompleteness::Incomplete { .. }
            ) {
                state.timeline.completeness = ScreenCompleteness::Incomplete { reason };
            }
        }
    }
    pub(crate) fn snapshot(&self) -> ScreenTimeline {
        self.0
            .lock()
            .map(|state| state.timeline.clone())
            .unwrap_or_else(|_| ScreenTimeline {
                duration_us: 0,
                refinement: ScreenRefinement::Pending,
                diagnostics: ScreenDiagnostics::default(),
                frames: Vec::new(),
                completeness: ScreenCompleteness::Incomplete {
                    reason: CaptureFailure::Unavailable,
                },
            })
    }
}

/// 队列最多 64 项且未完成像素最多 128MiB。
pub(crate) struct ScreenArchiveWriter {
    directory: PathBuf,
    sender: Option<SyncSender<ArchiveJob>>,
    threads: Vec<JoinHandle<()>>,
    bytes: Arc<AtomicUsize>,
    items: Arc<AtomicUsize>,
    result: ArchiveResult,
}

impl ScreenArchiveWriter {
    pub(crate) fn start(directory: PathBuf) -> Result<Self, crate::RecorderError> {
        let (sender, receiver) = mpsc::sync_channel::<ArchiveJob>(64);
        let receiver = Arc::new(Mutex::new(receiver));
        let bytes = Arc::new(AtomicUsize::new(0));
        let items = Arc::new(AtomicUsize::new(0));
        let result = ArchiveResult(Arc::new(Mutex::new(CommitState {
            next: 1,
            ready: BTreeMap::new(),
            timeline: ScreenTimeline::default(),
        })));
        let mut threads = Vec::new();
        for index in 0..2 {
            let receiver = receiver.clone();
            let bytes = bytes.clone();
            let items = items.clone();
            let result = result.clone();
            let directory = directory.clone();
            let thread = std::thread::Builder::new()
                .name(format!("argusflow-screen-png-{index}"))
                .spawn(move || {
                    loop {
                        let job = match receiver.lock() {
                            Ok(receiver) => receiver.recv(),
                            Err(_) => break,
                        };
                        let Ok(job) = job else {
                            break;
                        };
                        let id = job.record.id.0;
                        let encoded = encode(&directory, &job);
                        bytes.fetch_sub(job.bytes, Ordering::AcqRel);
                        items.fetch_sub(1, Ordering::AcqRel);
                        if let Ok(mut state) = result.0.lock() {
                            state.ready.insert(id, encoded);
                            loop {
                                let next = state.next;
                                let Some(record) = state.ready.remove(&next) else {
                                    break;
                                };
                                match record {
                                    Ok(record) => {
                                        state.timeline.duration_us =
                                            state.timeline.duration_us.max(record.presented_us);
                                        state.timeline.frames.push(record);
                                    }
                                    Err(reason) => {
                                        state.timeline.completeness =
                                            ScreenCompleteness::Incomplete { reason };
                                        // 失败位置不能被后来的成功项跨越。
                                        break;
                                    }
                                }
                                state.next += 1;
                            }
                        }
                    }
                })
                .map_err(|_| crate::RecorderError::WorkerUnavailable)?;
            threads.push(thread);
        }
        Ok(Self {
            directory,
            sender: Some(sender),
            threads,
            bytes,
            items,
            result,
        })
    }

    pub(crate) fn result(&self) -> ArchiveResult {
        self.result.clone()
    }

    pub(crate) fn submit(
        &self,
        frame: FrameUpdate,
        record: ScreenFrame,
        regions: Vec<PixelRect>,
    ) -> Result<(), CaptureFailure> {
        // 计入被作业持有的整份快照，宁可保守限制也不隐性超额。
        let bytes = frame.snapshot.byte_len();
        let budget = 128 * 1024 * 1024;
        let items = self
            .items
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                (used < 64).then_some(used + 1)
            })
            .map_err(|_| CaptureFailure::Capacity)?
            + 1;
        let reserved = self
            .bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes).filter(|total| *total <= budget)
            })
            .map_err(|_| CaptureFailure::Capacity);
        let used_bytes = match reserved {
            Ok(previous) => previous + bytes,
            Err(reason) => {
                self.items.fetch_sub(1, Ordering::AcqRel);
                return Err(reason);
            }
        };
        if let Ok(mut state) = self.result.0.lock() {
            state.timeline.diagnostics.queue_peak_items =
                state.timeline.diagnostics.queue_peak_items.max(items);
            state.timeline.diagnostics.queue_peak_bytes =
                state.timeline.diagnostics.queue_peak_bytes.max(used_bytes);
        }
        let job = ArchiveJob {
            frame,
            record,
            regions,
            bytes,
        };
        if self
            .sender
            .as_ref()
            .is_none_or(|sender| sender.try_send(job).is_err())
        {
            self.bytes.fetch_sub(bytes, Ordering::AcqRel);
            self.items.fetch_sub(1, Ordering::AcqRel);
            return Err(CaptureFailure::Capacity);
        }
        Ok(())
    }
}

impl Drop for ScreenArchiveWriter {
    fn drop(&mut self) {
        self.sender.take();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
        // 删除未进入有效前缀的区域，避免失败作业留下无引用原始像素。
        let timeline = self.result.snapshot();
        if let Ok(entries) = std::fs::read_dir(&self.directory) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                let Some(id) = name
                    .strip_prefix("screen-")
                    .and_then(|value| value.split('-').next())
                    .and_then(|id| id.parse::<u64>().ok())
                else {
                    continue;
                };
                if !timeline.frames.iter().any(|frame| frame.id.0 == id)
                    && entry.file_type().is_ok_and(|kind| kind.is_file())
                {
                    if std::fs::remove_file(entry.path()).is_err() {
                        self.result.fail(CaptureFailure::Storage);
                    }
                }
            }
        }
    }
}

fn encode(directory: &std::path::Path, job: &ArchiveJob) -> Result<ScreenFrame, CaptureFailure> {
    let mut record = job.record.clone();
    for (index, region) in job.regions.iter().enumerate() {
        let frame = job
            .frame
            .snapshot
            .crop(*region)
            .map_err(|_| CaptureFailure::Storage)?
            .into_rgba8();
        let name = patch_name(record.id, index as u32);
        let target = directory.join(&name);
        let pending = target.with_extension("pending");
        let file = std::fs::File::create(&pending).map_err(|_| CaptureFailure::Storage)?;
        let mut encoder =
            png::Encoder::new(std::io::BufWriter::new(file), frame.width(), frame.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder
            .write_header()
            .map_err(|_| CaptureFailure::Storage)?;
        writer
            .write_image_data(frame.pixels())
            .map_err(|_| CaptureFailure::Storage)?;
        writer.finish().map_err(|_| CaptureFailure::Storage)?;
        std::fs::rename(pending, target).map_err(|_| CaptureFailure::Storage)?;
        record.patches.push(ScreenPatch {
            index: index as u32,
            bounds: InspectionRect {
                x: frame.bounds().x,
                y: frame.bounds().y,
                width: frame.width().into(),
                height: frame.height().into(),
            },
        });
    }
    Ok(record)
}
