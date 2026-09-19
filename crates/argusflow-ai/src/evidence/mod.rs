//! 正式录制包的只读索引；不依赖 demo 私有事实或预期步骤。
mod images;
mod timeline;
use crate::{AiError, Result};
use argusflow_recorder::{
    Cursor, Record, RecordData, Session, SessionPhase, load_session, read_page,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
/// 有界且冻结的录制证据集。
pub struct Evidence {
    pub(crate) directory: PathBuf,
    pub(crate) session: Session,
    pub(crate) records: BTreeMap<u64, Record>,
}
impl Evidence {
    /// 原始会话身份，用于把生成节点关联回录制历史。
    pub fn session_id(&self) -> &str {
        &self.session.id
    }
    /// 只接受已停止的完整录制，不在活跃日志上推测结尾。
    pub fn load(directory: &Path) -> Result<Self> {
        let session = load_session(directory).map_err(|e| AiError::Invalid(e.to_string()))?;
        let mut cursor = Cursor::default();
        let mut records = BTreeMap::new();
        let mut bytes = 0usize;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if std::time::Instant::now() > deadline {
                return Err(AiError::Budget("读取录制超时".into()));
            }
            let page =
                read_page(directory, cursor, 256).map_err(|e| AiError::Invalid(e.to_string()))?;
            if page.tail.is_some() {
                return Err(AiError::Invalid("录制日志未完整收尾".into()));
            }
            cursor = page.cursor;
            for record in page.records {
                bytes += serde_json::to_vec(&record)?.len();
                if bytes > 24 * 1024 * 1024 || records.len() >= 30000 {
                    return Err(AiError::Budget(
                        "录制超过 30000 条或 24 MiB，请缩短录制".into(),
                    ));
                }
                records.insert(record.id, record);
            }
            if page.end {
                break;
            }
        }
        let stopped = records.values().rev().find_map(|r| match &r.data {
            RecordData::State { phase, .. } => Some(*phase),
            _ => None,
        });
        if stopped != Some(SessionPhase::Stopped) {
            return Err(AiError::Invalid("请先正常停止录制，再整理工作流".into()));
        }
        Ok(Self {
            directory: directory.to_owned(),
            session,
            records,
        })
    }
    /// 必须在节点引用或未决问题中解释的操作。
    pub(crate) fn required_ids(&self) -> Vec<String> {
        let consumed: std::collections::BTreeSet<_> = self
            .records
            .values()
            .filter_map(|r| match &r.data {
                RecordData::Interaction(i) => Some(&i.raw),
                _ => None,
            })
            .flatten()
            .copied()
            .collect();
        self.records
            .values()
            .filter(|r| {
                matches!(r.data, RecordData::Interaction(_))
                    || matches!(r.data, RecordData::Raw(_)) && !consumed.contains(&r.id)
            })
            .map(|r| r.id.to_string())
            .collect()
    }
    pub(crate) fn inspect(&self, ids: &[String]) -> Result<serde_json::Value> {
        if ids.is_empty() || ids.len() > 6 {
            return Err(AiError::Invalid("每次查询 1–6 个证据 ID".into()));
        }
        let facts = ids
            .iter()
            .map(|id| {
                let record = self
                    .records
                    .get(
                        &id.parse::<u64>()
                            .map_err(|_| AiError::Invalid("证据 ID 无效".into()))?,
                    )
                    .ok_or_else(|| AiError::Invalid("证据 ID 不存在".into()))?;
                let mut value = serde_json::to_value(record)?;
                // 不把密码结构的潜在敏感字段发给模型。
                if matches!(&record.data,RecordData::Structure(s) if s.sensitive) {
                    value = serde_json::json!({"id":id,"sensitive_omitted":true});
                }
                Ok(value)
            })
            .collect::<Result<Vec<_>>>()?;
        let value = serde_json::json!({"facts":facts});
        if serde_json::to_vec(&value)?.len() > 64000 {
            return Err(AiError::Budget("单次证据超过 64 KiB，请减少 ID".into()));
        }
        Ok(value)
    }
}
