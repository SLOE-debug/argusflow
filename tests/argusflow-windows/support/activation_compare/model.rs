//! 策略清单与一轮观察；成功由实际前台状态决定，不由 API 返回值决定。
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    Direct,
    Alt,
    MouseMove,
    Attach,
    Switch,
    Project,
}
impl Strategy {
    pub const ALL: [Self; 6] = [
        Self::Direct,
        Self::Alt,
        Self::MouseMove,
        Self::Attach,
        Self::Switch,
        Self::Project,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Alt => "alt",
            Self::MouseMove => "mouse_move",
            Self::Attach => "attach",
            Self::Switch => "switch",
            Self::Project => "project",
        }
    }
}
impl FromStr for Strategy {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|s| s.name() == value)
            .ok_or_else(|| format!("未知策略：{value}"))
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub foreground: isize,
    pub foreground_pid: u32,
    pub focus: isize,
    pub gui_flags: u32,
    pub cursor: [i32; 2],
    pub alt_down: bool,
    pub target_topmost: bool,
    pub target_minimized: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Attempt {
    pub strategy: Strategy,
    pub target: isize,
    pub worker_pid: u32,
    pub before: Snapshot,
    pub after: Snapshot,
    pub api_ms: f64,
    pub elapsed_ms: f64,
    pub foreground_stable: bool,
    pub api_trace: Vec<String>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Trial {
    pub case: String,
    pub restriction: String,
    pub round: usize,
    pub strategy: Strategy,
    pub prepared_foreground: isize,
    pub lock_accepted: bool,
    pub attempt: Option<Attempt>,
    pub worker_timeout: bool,
    pub error: Option<String>,
}
