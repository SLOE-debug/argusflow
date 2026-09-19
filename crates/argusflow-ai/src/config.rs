//! 可序列化的模型配置与只写密钥更新。
use crate::{AiError, Result};
use serde::{Deserialize, Serialize};
/// 百炼兼容接口配置；预算按一次分析生效。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiConfig {
    /// 完整 chat/completions HTTPS 地址。
    pub endpoint: String,
    /// 模型服务中的精确模型名称。
    pub model: String,
    /// 是否允许发送录制图片；关闭后只有事实文本。
    pub vision: bool,
    /// 是否发送百炼 enable_thinking=false 扩展。
    pub bailian: bool,
    /// 最多对话轮数，包含纠错轮。
    pub max_rounds: u8,
    /// 累计只读工具调用次数。
    pub max_tools: u8,
    /// 单轮输出 token 上限。
    pub max_tokens: u32,
    /// 单轮网络超时秒数。
    pub timeout_seconds: u64,
}
impl Default for AiConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions".into(),
            model: "qwen3.7-plus-2026-05-26".into(),
            vision: true,
            bailian: true,
            max_rounds: 4,
            max_tools: 6,
            max_tokens: 16000,
            timeout_seconds: 180,
        }
    }
}
impl AiConfig {
    /// 配置在落盘和请求前均验证，不自动改写服务地址。
    pub fn validate(&self) -> Result<()> {
        let url = reqwest::Url::parse(&self.endpoint)
            .map_err(|_| AiError::Invalid("模型地址无效".into()))?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(AiError::Invalid(
                "模型地址须为不含凭据、查询参数和片段的 HTTPS 地址".into(),
            ));
        }
        if self.model.trim().is_empty()
            || self.model.len() > 128
            || !(1..=6).contains(&self.max_rounds)
            || self.max_tools > 12
            || !(256..=16000).contains(&self.max_tokens)
            || !(10..=240).contains(&self.timeout_seconds)
        {
            return Err(AiError::Invalid("模型或预算配置无效".into()));
        }
        Ok(())
    }
}
/// 前端只获得密钥是否已设置，不获得已保存的原文。
#[derive(Clone, Serialize, Deserialize)]
pub struct ConfigView {
    /// 可编辑的非敏感配置。
    pub config: AiConfig,
    /// 数据库是否已保存密钥。
    pub has_key: bool,
}
/// 密钥为 null 保持原值，空字符串删除；端点更换必须提供新密钥。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveConfig {
    /// 完整配置。
    pub config: AiConfig,
    /// 可选只写密钥。
    pub api_key: Option<String>,
}
