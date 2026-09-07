//! Win32 Hook 的 u32 毫秒时钟展开，保证录制跨 GetTickCount 回绕时不倒退。

/// 只由 ingestion 线程持有，基于真实事件时间而非语义解析完成时间。
pub(crate) struct EventClock {
    /// 上一个 Win32 原始 tick。
    previous: u32,
    /// 从录制开始累计的毫秒数。
    elapsed: u64,
}

impl EventClock {
    pub(crate) fn new(started_tick: u32) -> Self {
        Self {
            previous: started_tick,
            elapsed: 0,
        }
    }

    pub(crate) fn advance(&mut self, tick: u32) -> u64 {
        self.elapsed = self
            .elapsed
            .saturating_add(u64::from(tick.wrapping_sub(self.previous)));
        self.previous = tick;
        self.elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_tick_wrap_and_repeated_timestamps_preserve_elapsed_time() {
        let mut clock = EventClock::new(u32::MAX - 5);
        assert_eq!(clock.advance(u32::MAX - 1), 4);
        assert_eq!(clock.advance(3), 9);
        assert_eq!(clock.advance(3), 9);
        assert_eq!(clock.advance(13), 19);
    }
}
