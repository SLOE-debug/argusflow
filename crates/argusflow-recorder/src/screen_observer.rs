//! 唯一桌面画面时间轴；所有输入事件共享采样和 diff 结果。
use argusflow_core::{EvidenceFrame, InspectionFailure};
use std::{collections::VecDeque, sync::Arc};

/// 只读帧引用，同一屏幕状态不重复保存整份像素。
#[derive(Clone)]
pub(crate) struct ScreenSample {
    /// 相对上一采样的实际变化块，使用屏幕坐标。
    pub changes: Arc<[argusflow_core::InspectionRect]>,
    pub frame: Arc<EvidenceFrame>,
    /// 实际采样完成时间，确保点击前证据不会选到采样期间发生点击的帧。
    pub observed_ms: u64,
    pub duration_ms: u64,
    /// 像素或几何改变才递增；多个事件共享此变化代数。
    pub revision: u64,
}

/// 最多 16 个时间点且像素预算 128MiB；历史不落盘，停止时释放。
#[derive(Default)]
pub(crate) struct ScreenHistory {
    samples: VecDeque<ScreenSample>,
    revision: u64,
}
impl ScreenHistory {
    /// 平台确认没有更新时只追加时间点，不读回或比较像素。
    pub(crate) fn unchanged(&mut self, observed_ms: u64) -> Option<ScreenSample> {
        let mut sample = self.samples.back()?.clone();
        sample.observed_ms = observed_ms;
        sample.duration_ms = 0;
        sample.changes = Arc::from([]);
        let capacity = (128 * 1024 * 1024 / sample.frame.pixels().len()).clamp(1, 16);
        while self.samples.len() >= capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(sample.clone());
        Some(sample)
    }
    pub(crate) fn latest_frame(&self) -> Option<Arc<EvidenceFrame>> {
        self.samples.back().map(|sample| sample.frame.clone())
    }

    #[cfg(test)]
    pub(crate) fn push(
        &mut self,
        frame: EvidenceFrame,
        observed_ms: u64,
        duration_ms: u64,
    ) -> ScreenSample {
        let changed = self
            .samples
            .back()
            .is_none_or(|last| crate::screen_diff::changed_tiles(&last.frame, &frame) != 0);
        let regions = crate::evidence_region::changes(self.latest_frame().as_deref(), &frame);
        self.push_changed(frame, observed_ms, duration_ms, changed, regions)
    }

    /// diff 在锁外完成；发布时只做有界队列和 Arc 操作。
    pub(crate) fn push_changed(
        &mut self,
        frame: EvidenceFrame,
        observed_ms: u64,
        duration_ms: u64,
        changed: bool,
        regions: Vec<argusflow_core::InspectionRect>,
    ) -> ScreenSample {
        if changed {
            self.revision += 1;
        }
        let frame = if changed {
            Arc::new(frame)
        } else {
            // unchanged 只有存在上一帧时成立。
            self.samples
                .back()
                .map(|sample| sample.frame.clone())
                .unwrap_or_else(|| Arc::new(frame))
        };
        let capacity = (128 * 1024 * 1024 / frame.pixels().len()).clamp(1, 16);
        while self.samples.len() >= capacity {
            self.samples.pop_front();
        }
        let sample = ScreenSample {
            changes: regions.into(),
            frame,
            observed_ms,
            duration_ms,
            revision: self.revision,
        };
        self.samples.push_back(sample.clone());
        sample
    }

    /// 必须发生在输入之前；允许 200ms 的帧龄，绝不以稍后结果替代缺失目标。
    pub(crate) fn before(&self, event_ms: u64) -> Result<ScreenSample, InspectionFailure> {
        self.samples
            .iter()
            .rev()
            .find(|sample| sample.observed_ms <= event_ms && event_ms - sample.observed_ms <= 200)
            .cloned()
            .ok_or(InspectionFailure::Unavailable)
    }

    pub(crate) fn clear(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_click_keeps_preceding_frame_when_notepad_replaces_it() {
        let frame = |value| {
            EvidenceFrame::new(
                argusflow_core::InspectionRect {
                    x: 0.0,
                    y: 0.0,
                    width: 2.0,
                    height: 2.0,
                },
                2,
                2,
                argusflow_core::EvidencePixelFormat::Rgba8,
                vec![value; 16],
            )
            .unwrap()
        };
        let mut history = ScreenHistory::default();
        let search = history.push(frame(10), 100, 1);
        let same = history.push(frame(10), 140, 1);
        assert!(Arc::ptr_eq(&search.frame, &same.frame));
        let notepad = history.push(frame(20), 190, 1);
        assert!(notepad.revision > search.revision);
        assert_eq!(history.before(150).unwrap().frame.pixels(), &[10; 16]);
        assert!(history.before(99).is_err());
        assert!(history.before(400).is_err());
    }
}
