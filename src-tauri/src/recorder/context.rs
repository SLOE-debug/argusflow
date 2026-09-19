//! 按时间线原始事件读取关联操作，独立于前端分页缓存。
use argusflow_recorder::*;
use std::{collections::HashSet, path::Path};

/// 单项操作及其关联证据、相邻操作边界。
#[derive(serde::Serialize)]
pub(crate) struct OperationContext {
    pub action: Record,
    pub records: Vec<Record>,
}

#[cfg(test)]
#[path = "../../../tests/argusflow-desktop/unit/recorder/context.rs"]
mod tests;

/// 两次顺序扫描只保留当前操作来源和证据，限制返回体积。
pub(super) fn read(path: &Path, raw_id: u64) -> Result<OperationContext, String> {
    let mut action = None;
    let mut raw = None;
    scan(path, |record| {
        if record.id == raw_id && matches!(record.data, RecordData::Raw(_)) {
            raw = Some(record.clone());
        }
        if let RecordData::Interaction(value) = &record.data
            && value.raw.contains(&raw_id)
        {
            action = Some(record);
        }
        Ok(())
    })?;
    let action = action.or(raw).ok_or("原始输入记录不存在")?;
    let (ids, through): (HashSet<_>, _) = match &action.data {
        RecordData::Interaction(value) => (
            value.raw.iter().copied().chain(value.related).collect(),
            value.through_qpc,
        ),
        RecordData::Raw(value) => (HashSet::from([action.id]), value.qpc),
        _ => return Err("操作记录类型无效".into()),
    };
    let end = through.saturating_add(
        load_session(path)
            .map_err(|e| e.to_string())?
            .qpc_frequency
            .saturating_mul(2) as i64,
    );
    let mut result = Vec::new();
    let mut bytes = 0;
    scan(path, |record| {
        let belongs = match &record.data {
            RecordData::Interaction(value) => value.from_qpc > through && value.from_qpc <= end,
            RecordData::Raw(_) => ids.contains(&record.id),
            RecordData::Structure(value) => value.raw.iter().any(|id| ids.contains(id)),
            RecordData::Clipboard(value) => value.raw.iter().any(|id| ids.contains(id)),
            RecordData::Attempt { raw, .. } => raw.iter().any(|id| ids.contains(id)),
            _ => false,
        };
        if belongs {
            bytes += serde_json::to_vec(&record)
                .map_err(|e| e.to_string())?
                .len();
            if bytes > 8 * 1024 * 1024 {
                return Err("此操作的控件信息超过读取预算".into());
            }
            result.push(record);
        }
        Ok(())
    })?;
    Ok(OperationContext {
        action,
        records: result,
    })
}

fn scan(path: &Path, mut visit: impl FnMut(Record) -> Result<(), String>) -> Result<(), String> {
    let mut cursor = Cursor::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if std::time::Instant::now() > deadline {
            return Err("读取操作信息超时".into());
        }
        let page = read_page(path, cursor, 256).map_err(|e| e.to_string())?;
        cursor = page.cursor;
        for record in page.records {
            visit(record)?;
        }
        if page.end || page.tail.is_some() {
            return Ok(());
        }
    }
}
