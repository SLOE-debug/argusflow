//! 平台契约失序时关闭读取，绝不把迟到像素写到新版本索引。
use super::State;
use argusflow_capture_contracts::*;
use argusflow_core::FailureKind;

impl State {
    pub(super) fn validate_event(&self, event: &BackendEvent) -> CaptureResult<()> {
        if let BackendEvent::Source(info) = event {
            if !self.sources.contains_key(&info.id) && self.sources.len() >= 512 {
                return Err(CaptureError::new(
                    FailureKind::ResourceLimit,
                    "capture_sources",
                    "会话累计来源超过 512 个",
                ));
            }
            if info.name.len() > 4096 {
                return Err(protocol("来源名称超限"));
            }
        }
        let (snapshot, changes) = match event {
            BackendEvent::Baseline(snapshot) => (snapshot, None),
            BackendEvent::Changed { snapshot, changes } => (snapshot, Some(changes)),
            _ => return Ok(()),
        };
        let entry = self
            .sources
            .get(&snapshot.version.source)
            .ok_or_else(|| protocol("快照没有来源描述"))?;
        if snapshot.version.session != self.session
            || snapshot.version.generation != entry.info.generation
            || snapshot.bounds != entry.info.bounds
            || snapshot
                .timing
                .presented
                .is_some_and(|time| time > snapshot.timing.acquired)
            || snapshot.timing.acquired > snapshot.timing.frozen
        {
            return Err(protocol("快照会话、代际、边界或时间不一致"));
        }
        if let Some(changes) = changes {
            let previous = entry
                .history
                .back()
                .ok_or_else(|| protocol("变化缺少此前基线"))?;
            if previous.version.revision.checked_add(1) != Some(snapshot.version.revision)
                || previous
                    .timing
                    .presented
                    .unwrap_or(previous.timing.acquired)
                    > snapshot
                        .timing
                        .presented
                        .unwrap_or(snapshot.timing.acquired)
                || changes.changed_pixels == 0
                || changes.changed_pixels > changes.compared_pixels
                || changes.regions.is_empty()
                || changes.regions.len() > 262144
                || changes
                    .regions
                    .iter()
                    .any(|region| !snapshot.bounds.local().contains(*region))
            {
                return Err(protocol("变化修订号、时间或精确差分元数据失序"));
            }
        }
        Ok(())
    }
}
fn protocol(message: &'static str) -> CaptureError {
    CaptureError::new(FailureKind::Protocol, "capture_backend_event", message)
}
