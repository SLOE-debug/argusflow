//! 单次分析票据，取消只影响对应请求；退出与异常释放占用。
use argusflow_ai::CancellationToken;
use std::sync::Mutex;
#[derive(Default)]
pub(crate) struct AiJobs {
    active: Mutex<Option<(String, CancellationToken)>>,
}
pub(crate) struct Job<'a> {
    owner: &'a AiJobs,
    pub token: CancellationToken,
}
impl AiJobs {
    pub fn begin(&self, id: String) -> Result<Job<'_>, String> {
        let mut active = self.active.lock().map_err(|_| "AI 任务状态不可用")?;
        if active.is_some() {
            return Err("已有 AI 整理任务正在进行".into());
        }
        let token = CancellationToken::new();
        *active = Some((id, token.clone()));
        Ok(Job { owner: self, token })
    }
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        let active = self.active.lock().map_err(|_| "AI 任务状态不可用")?;
        if let Some((current, token)) = active.as_ref()
            && current == id
        {
            token.cancel();
        }
        Ok(())
    }
}
impl Drop for Job<'_> {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.owner.active.lock() {
            *slot = None;
        }
    }
}
