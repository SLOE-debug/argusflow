//! 跨焦点建立与多次注入的独占输入租约。
use super::{InputAction, InputService};
use crate::{WindowIdentity, WindowsError};
use argusflow_core::Operation;

/// 同一输入服务最多存在一个序列；释放不会重复或撤销已经发送的输入。
pub struct InputSequence<'a> {
    service: &'a InputService,
    operation: &'a Operation,
}
impl<'a> InputSequence<'a> {
    pub(super) fn new(service: &'a InputService, operation: &'a Operation) -> Self {
        Self { service, operation }
    }
    /// 顺序发送一次输入，沿用整个动作的截止时间与副作用状态。
    pub async fn perform(
        &mut self,
        window: WindowIdentity,
        action: InputAction,
    ) -> Result<(), WindowsError> {
        self.service.submit(window, action, self.operation).await
    }
}
impl Drop for InputSequence<'_> {
    fn drop(&mut self) {
        self.service.release_sequence();
    }
}
