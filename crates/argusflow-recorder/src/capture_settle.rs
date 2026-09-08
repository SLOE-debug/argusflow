//! 输入事件的稳定区间只依赖共享画面代数，不重复读取或比较像素。
use crate::PhysicalInput;
pub(crate) struct CaptureSettle {
    minimum_ms: u64,
    previous_revision: Option<u64>,
    stable_since_ms: u64,
    changed: bool,
}
impl CaptureSettle {
    pub(crate) fn new(input: PhysicalInput, baseline: Option<u64>) -> Self {
        Self {
            minimum_ms: match input {
                PhysicalInput::Key { .. } | PhysicalInput::Window { .. } => 500,
                _ => 250,
            },
            previous_revision: baseline,
            stable_since_ms: 0,
            changed: false,
        }
    }
    /// 同一帧共享 revision；观察到像素变化后，要求最小等待之后连续安静 150ms。
    pub(crate) fn observe(&mut self, age_ms: u64, revision: u64) -> bool {
        if self.previous_revision != Some(revision) {
            self.changed |= self.previous_revision.is_some();
            self.previous_revision = Some(revision);
            self.stable_since_ms = age_ms;
        }
        self.changed && age_ms.saturating_sub(self.stable_since_ms.max(self.minimum_ms)) >= 150
    }
    /// 采样失败打断稳定区间，恢复后必须重新观察到变化。
    pub(crate) fn invalidate(&mut self) {
        self.previous_revision = None;
        self.changed = false;
    }
}
