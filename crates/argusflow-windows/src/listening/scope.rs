//! 自身窗口过滤只排除本地操作；Win 键属于桌面导航，不属于前台应用。
use argusflow_input_contracts::InputKind;

pub(super) fn accepts(kind: InputKind, pid: u32, excluded: u32, included: Option<u32>) -> bool {
    if included.is_some_and(|target| target != pid) {
        return false;
    }
    pid != excluded || matches!(kind, InputKind::Key { vk: 91 | 92, .. })
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/listening/scope.rs"]
mod tests;
