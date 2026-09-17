//! 不从键码推断最终文本；状态边界清空所有手势。
use crate::{Interaction, InteractionKind as Kind, RecordId};
use argusflow_input_contracts::{Button, InputEvent, InputKind, WindowContext};
use std::collections::{BTreeMap, BTreeSet};

struct Gesture {
    event: InputEvent,
    raw: Vec<RecordId>,
    moved: bool,
}
/// 单会话归一化状态，不跨前台代际或暂停边界合并。
pub struct Normalizer {
    frequency: u64,
    window: Option<WindowContext>,
    buttons: BTreeMap<Button, Gesture>,
    keys: BTreeSet<u32>,
    key_raw: Vec<RecordId>,
    last_click: Option<(InputEvent, RecordId, Button)>,
    double_ms: u32,
    distance: i32,
}
impl Normalizer {
    /// Windows 的双击时间与距离由平台装配传入。
    pub fn new(frequency: u64, double_ms: u32, distance: i32) -> Self {
        Self {
            frequency,
            window: None,
            buttons: BTreeMap::new(),
            keys: BTreeSet::new(),
            key_raw: vec![],
            last_click: None,
            double_ms,
            distance,
        }
    }
    /// 用户状态边界必须立即调用，不把跨边界释放解释成点击。
    pub fn reset(&mut self) {
        self.window = None;
        self.buttons.clear();
        self.keys.clear();
        self.key_raw.clear();
        self.last_click = None;
    }
    /// 消费已经确认写入的原始记录，返回零或一项派生操作。
    pub fn push(&mut self, id: RecordId, event: InputEvent) -> Option<Interaction> {
        if self.window != Some(event.window) {
            self.reset();
            self.window = Some(event.window);
        }
        let mut raw = vec![id];
        let mut from = event.qpc;
        let mut related = None;
        let kind = match event.kind {
            InputKind::Context => Kind::WindowSwitch,
            InputKind::Move => {
                for gesture in self.buttons.values_mut() {
                    gesture.moved |= distance(gesture.event, event) > self.distance;
                    if gesture.raw.len() < 2047 {
                        gesture.raw.push(id);
                    }
                }
                return None;
            }
            InputKind::Button { button, down: true } => {
                self.buttons.insert(
                    button,
                    Gesture {
                        event,
                        raw,
                        moved: false,
                    },
                );
                return None;
            }
            InputKind::Button {
                button,
                down: false,
            } => match self.buttons.remove(&button) {
                None => Kind::Unresolved,
                Some(mut gesture) => {
                    gesture.raw.push(id);
                    raw = gesture.raw;
                    from = gesture.event.qpc;
                    if gesture.moved || distance(gesture.event, event) > self.distance {
                        self.last_click = None;
                        Kind::Drag
                    } else {
                        let double = self.last_click.is_some_and(|(last, _, b)| {
                            b == button
                                && last.window == event.window
                                && distance(last, event) <= self.distance
                                && event.qpc >= last.qpc
                                && (event.qpc - last.qpc) as u128 * 1000
                                    <= u128::from(self.frequency) * u128::from(self.double_ms)
                        });
                        if double {
                            related = self.last_click.take().map(|(_, id, _)| id);
                            Kind::DoubleClick
                        } else {
                            self.last_click = Some((event, id, button));
                            Kind::Click
                        }
                    }
                }
            },
            InputKind::Wheel { .. } => {
                self.last_click = None;
                Kind::Scroll
            }
            InputKind::Key { vk, down, .. } => {
                self.last_click = None;
                if !down {
                    self.keys.remove(&vk);
                    if self.keys.is_empty() {
                        self.key_raw.clear();
                    }
                    if matches!(vk, 91 | 92) {
                        // 前台可能已经因 Win 键切换；仅引用本次释放事实，不跨窗口拼装按键。
                        raw = vec![id];
                        Kind::SystemKey
                    } else {
                        return None;
                    }
                } else {
                    self.keys.insert(vk);
                    if self.key_raw.len() >= 64 {
                        self.key_raw.clear();
                    }
                    self.key_raw.push(id);
                    raw = self.key_raw.clone();
                    let ctrl = self.keys.iter().any(|k| matches!(k, 17 | 162 | 163));
                    let modifier = matches!(vk, 16..=18 | 91 | 92 | 160..=165);
                    if modifier {
                        return None;
                    }
                    if vk == 229 {
                        Kind::ImeUnconfirmed
                    } else if ctrl && vk == 86 {
                        Kind::PasteUnconfirmed
                    } else if matches!(vk, 8 | 46) {
                        Kind::DeleteUnconfirmed
                    } else if ctrl
                        || self
                            .keys
                            .iter()
                            .any(|k| matches!(k, 18 | 91 | 92 | 164 | 165))
                    {
                        Kind::Chord
                    } else {
                        Kind::TextUnconfirmed
                    }
                }
            }
        };
        Some(Interaction { raw, kind, from_qpc: from, through_qpc: event.qpc, window:event.window, basis: "基于同窗口代际的系统消息时序；文本结果需独立结构观察确认；轨迹最多2048条，其余仍保留原始日志".into(), related })
    }
}
fn distance(a: InputEvent, b: InputEvent) -> i32 {
    (i64::from(a.point.x) - i64::from(b.point.x))
        .abs()
        .max((i64::from(a.point.y) - i64::from(b.point.y)).abs())
        .min(i64::from(i32::MAX)) as i32
}
