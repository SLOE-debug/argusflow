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

    #[test]
    fn timeline_uses_event_time_not_callback_order_or_window_priority() {
        use crate::{EventTimeline, RawInput, RawTraceEvent, WindowChange};
        let window = argusflow_core::WindowIdentity {
            handle: 1,
            process_id: 1,
        };
        let event = |sequence, elapsed_ms, input| RawTraceEvent {
            sequence,
            timestamp_ms: elapsed_ms as u32,
            elapsed_ms,
            input,
            evidence: None,
            diagnostics: vec![],
        };
        let mut timeline = EventTimeline {
            events: vec![
                event(
                    1,
                    1050,
                    RawInput::Clipboard {
                        sequence_number: 1,
                        content: crate::ClipboardContent::Empty,
                    },
                ),
                event(
                    2,
                    1010,
                    RawInput::Window {
                        window,
                        change: WindowChange::Appeared,
                    },
                ),
                event(
                    3,
                    1050,
                    RawInput::Window {
                        window,
                        change: WindowChange::Foreground,
                    },
                ),
            ],
        };
        timeline.compact_pointer_motion();
        assert_eq!(
            timeline
                .events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            [2, 1, 3]
        );
    }
}
