//! 先发布可读的候选归档，再原子替换精确索引，最后清理旧像素。
use crate::{EvidenceCollector, RecorderError, RecordingFiles, RecordingTrace, ScreenRefinement};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub(crate) async fn finalize(
    root: &Path,
    collector: Arc<EvidenceCollector>,
    trace: &mut RecordingTrace,
    obsolete: &mut Vec<PathBuf>,
) -> Result<RecordingFiles, RecorderError> {
    // GPU 故障、任务取消或进程退出时，原始候选仍有正式索引可以回看。
    let mut files = crate::storage::save(root, trace).await?;
    if !matches!(trace.screen.refinement, ScreenRefinement::Complete)
        && !trace.screen.frames.is_empty()
    {
        let directory = files.evidence_directory.clone();
        let raw = trace.screen.clone();
        let refined = tokio::task::spawn_blocking(move || {
            let mut differ = collector
                .pixel_differ()
                .map_err(|error| RecorderError::Refinement(error.to_string()))?;
            crate::screen_refinement::refine(&directory, &raw, differ.as_mut())
        })
        .await
        .map_err(|_| RecorderError::WorkerUnavailable)?;
        match refined {
            Ok(staged) => {
                let mut precise = trace.clone();
                precise.screen = staged.timeline.clone();
                crate::screen_association::associate(&mut precise);
                let (_, old) = staged.commit();
                obsolete.extend(old);
                *trace = precise;
                // 从开始发布索引起保留两代像素；部分文件发布失败时重试同一份 Trace。
                files = crate::storage::save(root, trace).await?;
            }
            Err(error) => {
                trace.screen.refinement = ScreenRefinement::Failed {
                    message: error.to_string(),
                };
                files = crate::storage::save(root, trace).await?;
            }
        }
    }
    while let Some(path) = obsolete.last() {
        match tokio::fs::remove_file(path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        obsolete.pop();
    }
    Ok(files)
}
