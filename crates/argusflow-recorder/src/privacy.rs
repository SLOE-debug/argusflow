//! 显式会话配置，默认保留原始事实。
use serde::{Deserialize, Serialize};
/// 独立配置遮盖范围；截图采用整帧不保存策略，避免猜测敏感区域。
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
pub struct RecordingPrivacy {
    /// 总开关。
    pub enabled: bool,
    /// 遮盖敏感或未知字段的键盘输入与结构化名称。
    pub sensitive_input: bool,
    /// 不保存剪贴板文本。
    pub clipboard: bool,
    /// 不保存完整截图和局部图。
    pub screenshots: bool,
}
impl RecordingPrivacy {
    pub(crate) fn input(self) -> bool {
        self.enabled && self.sensitive_input
    }
    pub(crate) fn clipboard(self) -> bool {
        self.enabled && self.clipboard
    }
    pub(crate) fn screenshots(self) -> bool {
        self.enabled && self.screenshots
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn master_switch_and_independent_scopes() {
        let raw = RecordingPrivacy::default();
        assert!(!raw.input() && !raw.clipboard() && !raw.screenshots());
        let disabled = RecordingPrivacy {
            enabled: false,
            sensitive_input: true,
            clipboard: true,
            screenshots: true,
        };
        assert!(!disabled.input() && !disabled.clipboard() && !disabled.screenshots());
        let input_only = RecordingPrivacy {
            enabled: true,
            sensitive_input: true,
            ..Default::default()
        };
        assert!(input_only.input() && !input_only.clipboard() && !input_only.screenshots());
    }
}
