//! 同一浏览器包装中的页面只能有一个拥有清理责任的工作流资源。
use argusflow_runtime::RunError;
use argusflow_workflow::ErrorKind;
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

pub(super) type Claims = Arc<Mutex<BTreeSet<String>>>;
pub(super) struct PageClaim {
    claims: Claims,
    target: String,
}
impl PageClaim {
    pub fn acquire(claims: &Claims, target: &str) -> Result<Self, RunError> {
        let mut held = claims
            .lock()
            .map_err(|_| RunError::new(ErrorKind::Contract, "页面所有权锁失效"))?;
        if !held.insert(target.into()) {
            return Err(RunError::new(
                ErrorKind::Busy,
                "页面已被另一个活跃作用域附加",
            ));
        }
        Ok(Self {
            claims: claims.clone(),
            target: target.into(),
        })
    }
}
impl Drop for PageClaim {
    fn drop(&mut self) {
        if let Ok(mut claims) = self.claims.lock() {
            claims.remove(&self.target);
        }
    }
}
