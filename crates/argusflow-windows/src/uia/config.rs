//! UIA 遍历、租约和 Provider 资源预算。
use std::time::Duration;

/// UIA 资源和超时上限。
#[derive(Debug, Clone)]
pub struct UiaConfig {
    /// 等待队列容量，默认 64。
    pub queue_capacity: usize,
    /// 单次最多访问的元素数量。
    pub max_nodes: usize,
    /// 最多返回和同时持有的元素租约数量。
    pub max_results: usize,
    /// 窗口内部最大遍历深度。
    pub max_depth: usize,
    /// 元素句柄租约时长，不因读取自动延长。
    pub lease_duration: Duration,
    /// Provider 连接超时。
    pub connection_timeout: Duration,
    /// 单次 Provider 事务超时。
    pub transaction_timeout: Duration,
}
impl Default for UiaConfig {
    fn default() -> Self {
        Self {
            queue_capacity: 64,
            max_nodes: 10_000,
            max_results: 256,
            max_depth: 64,
            lease_duration: Duration::from_secs(60),
            connection_timeout: Duration::from_secs(2),
            transaction_timeout: Duration::from_secs(5),
        }
    }
}
