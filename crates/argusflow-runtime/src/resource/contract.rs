//! 资源数据不进入 Value，平台句柄只对所属适配器可见。
use crate::TaskFuture;
use argusflow_core::Operation;
use std::any::Any;

/// 被作用域拥有的一个资源；异步关闭必须可重复调用以恢复失败的清理。
pub trait Resource: Any + Send + Sync {
    /// 与任务资源签名一致的稳定类型。
    fn resource_type(&self) -> &str;
    /// 适配器在已经验证类型后访问自身资源实现。
    fn as_any(&self) -> &dyn Any;
    /// 回收自建对象或脱离附加对象；完成前不得提前宣告释放。
    fn cleanup<'a>(&'a self, operation: &'a Operation) -> TaskFuture<'a, ()>;
}
