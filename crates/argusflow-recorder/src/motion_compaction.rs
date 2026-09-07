//! 时间线鼠标采样合并：只压缩连续观察事实，所有交互、证据与缺口都是硬边界。

use crate::{
    EventTimeline, InputPhase, MotionPoint, MouseButton, PointerMotion, RawInput, RawTraceEvent,
    RecordingDiagnostic,
};

/// 超过 200ms 没有新移动视为停顿，不跨停顿合并轨迹。
const PAUSE_MS: u64 = 200;
/// 单段最多 5 秒或 4096 个原始点，限制轨迹简化成本与详情体积。
const MAX_SPAN_MS: u64 = 5_000;
const MAX_SAMPLES: usize = 4096;

/// 停止录制与读取历史共用的幂等整理；保留事件身份及全部非移动证据。
pub(crate) fn compact(timeline: &mut EventTimeline) {
    let mut events = std::mem::take(&mut timeline.events);
    // 先排序再合并，迟到的窗口通知也能在真实时间处切开移动。
    events.sort_by_key(|event| (event.elapsed_ms, event.sequence));
    let mut output = Vec::new();
    let mut pending: Option<MotionRun> = None;
    let mut pressed_buttons = Vec::new();
    let mut previous_sequence = 0_u64;
    for event in events {
        let gap = previous_sequence.checked_add(1) != Some(event.sequence)
            || event.diagnostics.contains(&RecordingDiagnostic::InputGap);
        if gap {
            pressed_buttons.clear();
        }
        previous_sequence = event.last_source_sequence();
        let point = match event.input {
            RawInput::Move { point } if event.evidence.is_none() => Some(point),
            _ => None,
        };
        if let Some(point) = point {
            if pending
                .as_ref()
                .is_some_and(|run| gap || !run.can_extend(&event))
            {
                flush(&mut pending, &mut output);
            }
            if let Some(run) = &mut pending {
                run.push(&event, point);
            } else {
                pending = Some(MotionRun::new(event, point, pressed_buttons.clone()));
            }
        } else {
            flush(&mut pending, &mut output);
            match &event.input {
                RawInput::Mouse {
                    button,
                    phase: InputPhase::Down,
                    ..
                } => {
                    if !pressed_buttons.contains(button) {
                        pressed_buttons.push(*button);
                    }
                }
                RawInput::Mouse {
                    button,
                    phase: InputPhase::Up,
                    ..
                } => pressed_buttons.retain(|held| held != button),
                // 已合并轨迹不重复简化；按键观察随持久化事实恢复，保证整理幂等。
                RawInput::PointerMotion(motion) => {
                    pressed_buttons.clone_from(&motion.pressed_buttons)
                }
                _ => {}
            }
            output.push(event);
        }
    }
    flush(&mut pending, &mut output);
    timeline.events = output;
}

/// 正在收集的一段连续鼠标输入；所有点都来自实际事件。
struct MotionRun {
    /// 首事件保留原始序号、时钟与该段共同的诊断。
    first: RawTraceEvent,
    /// 有界原始点集，完成时才做几何简化。
    points: Vec<MotionPoint>,
    /// 最近源事件的结束时间与序号，用于边界检查。
    last: MotionPoint,
    /// 根据原始路径累加，不受几何简化影响。
    distance_px: f64,
    /// 从同一时间线已观察到的按下事件派生。
    pressed_buttons: Vec<MouseButton>,
}

impl MotionRun {
    fn new(
        first: RawTraceEvent,
        point: argusflow_core::ScreenPoint,
        pressed_buttons: Vec<MouseButton>,
    ) -> Self {
        let last = MotionPoint {
            sequence: first.sequence,
            elapsed_ms: first.elapsed_ms,
            point,
        };
        Self {
            first,
            points: vec![last],
            last,
            distance_px: 0.0,
            pressed_buttons,
        }
    }

    fn can_extend(&self, event: &RawTraceEvent) -> bool {
        self.last.sequence.checked_add(1) == Some(event.sequence)
            && event.elapsed_ms >= self.last.elapsed_ms
            && event.elapsed_ms - self.last.elapsed_ms <= PAUSE_MS
            && event.elapsed_ms - self.first.elapsed_ms <= MAX_SPAN_MS
            && self.points.len() < MAX_SAMPLES
            && event.diagnostics == self.first.diagnostics
    }

    fn push(&mut self, event: &RawTraceEvent, point: argusflow_core::ScreenPoint) {
        self.distance_px += (f64::from(point.x) - f64::from(self.last.point.x))
            .hypot(f64::from(point.y) - f64::from(self.last.point.y));
        self.last = MotionPoint {
            sequence: event.sequence,
            elapsed_ms: event.elapsed_ms,
            point,
        };
        self.points.push(self.last);
    }

    fn finish(mut self) -> RawTraceEvent {
        if self.points.len() > 1 {
            self.first.input = RawInput::PointerMotion(PointerMotion {
                end_sequence: self.last.sequence,
                ended_ms: self.last.elapsed_ms,
                sample_count: self.points.len(),
                distance_px: self.distance_px,
                pressed_buttons: self.pressed_buttons,
                points: crate::motion_simplification::simplify(self.points),
            });
        }
        self.first
    }
}

fn flush(pending: &mut Option<MotionRun>, output: &mut Vec<RawTraceEvent>) {
    if let Some(run) = pending.take() {
        output.push(run.finish());
    }
}
