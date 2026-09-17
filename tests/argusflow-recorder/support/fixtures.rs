use argusflow_input_contracts::*;
use argusflow_recorder::*;
pub fn session() -> Session {
    Session {
        id: "test-session".into(),
        format: 2,
        created_ms: 0,
        qpc_origin: 0,
        qpc_frequency: 1000,
        capture_session: Some("1".into()),
        policy: "test".into(),
    }
}
pub fn event(kind: InputKind, time: i64) -> InputEvent {
    InputEvent {
        sequence: time as u64,
        qpc: time,
        system_time: time as u32,
        point: Point { x: -20, y: 30 },
        window: WindowContext {
            handle: 1,
            pid: 2,
            epoch: 1,
        },
        foreground_handle: 1,
        origin: InputOrigin::System,
        flags: 0,
        kind,
    }
}
pub fn button(down: bool, time: i64) -> InputEvent {
    event(
        InputKind::Button {
            button: Button::Left,
            down,
        },
        time,
    )
}
pub fn key(vk: u32, down: bool, time: i64) -> InputEvent {
    event(
        InputKind::Key {
            vk,
            scan: vk,
            down,
            repeat: false,
            extended: false,
        },
        time,
    )
}
