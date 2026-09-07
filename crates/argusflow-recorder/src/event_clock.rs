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
        // 不同 Win32 事件源可乱序送达；小幅倒退是历史事件而不是 49 天回绕。
        let delta = tick.wrapping_sub(self.previous) as i32;
        if delta >= 0 {
            self.elapsed = self.elapsed.saturating_add(delta as u64);
            self.previous = tick;
            self.elapsed
        } else {
            self.elapsed.saturating_sub(u64::from(delta.unsigned_abs()))
        }
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

    #[test]
    fn delayed_window_notification_is_not_mistaken_for_a_clock_wrap() {
        let mut clock = EventClock::new(1000);
        assert_eq!(clock.advance(1030), 30);
        assert_eq!(clock.advance(1010), 10);
        assert_eq!(clock.advance(1040), 40);
    }
}
