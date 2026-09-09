//! 供有界 worker 测试使用的合成身份；不会访问真实 HWND。
use super::WindowIdentity;

impl WindowIdentity {
    pub(crate) fn test_identity() -> Self {
        Self {
            handle: 1,
            process_id: 1,
            process_created: 1,
            stamp: super::super::stamp::Stamp::test_stamp(),
        }
    }
}
