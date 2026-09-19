//! SQLite 是配置唯一来源；不读取环境变量，也不自动迁移 .env。
use crate::{AiConfig, AiError, ConfigView, Result, SaveConfig};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
/// 短生命周期同步存储，在宿主阻塞线程调用。
pub struct ConfigStore(Connection);
impl ConfigStore {
    /// 初始化固定 schema；所有写入使用 SQLite 事务。
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(3))?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS ai_configuration (id INTEGER PRIMARY KEY CHECK(id=1), configuration TEXT NOT NULL, api_key TEXT NOT NULL);")?;
        Ok(Self(connection))
    }
    /// 返回脱敏配置。
    pub fn view(&self) -> Result<ConfigView> {
        let (config, key) = self.load()?;
        Ok(ConfigView {
            config,
            has_key: !key.is_empty(),
        })
    }
    /// 仅模型调用边界使用完整配置；调用方不得记录返回的密钥。
    pub fn load(&self) -> Result<(AiConfig, String)> {
        let row: Option<(String, String)> = self
            .0
            .query_row(
                "SELECT configuration,api_key FROM ai_configuration WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match row {
            Some((config, key)) => Ok((serde_json::from_str(&config)?, key)),
            None => Ok((AiConfig::default(), String::new())),
        }
    }
    /// 原子保存；端点切换不能隐式复用其他服务的密钥。
    pub fn save(&mut self, update: SaveConfig) -> Result<ConfigView> {
        update.config.validate()?;
        let transaction = self.0.transaction()?;
        let old: Option<(String, String)> = transaction
            .query_row(
                "SELECT configuration,api_key FROM ai_configuration WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if update.api_key.is_none()
            && let Some((config, _)) = &old
        {
            let previous: AiConfig = serde_json::from_str(config)?;
            if previous.endpoint != update.config.endpoint {
                return Err(AiError::Invalid("更换服务地址时请重新填写 API Key".into()));
            }
        }
        let key = update
            .api_key
            .unwrap_or_else(|| old.map(|(_, key)| key).unwrap_or_default());
        if key.len() > 4096 || key.chars().any(char::is_control) || key.trim() != key {
            return Err(AiError::Invalid("API Key 包含空白或无效字符".into()));
        }
        transaction.execute("INSERT INTO ai_configuration VALUES(1,?1,?2) ON CONFLICT(id) DO UPDATE SET configuration=excluded.configuration,api_key=excluded.api_key", params![serde_json::to_string(&update.config)?,key])?;
        transaction.commit()?;
        self.view()
    }
}
