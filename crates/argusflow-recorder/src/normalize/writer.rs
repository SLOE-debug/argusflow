//! 单写者将事实提交与派生追加排序；不调用任何证据服务。
use crate::*;
use argusflow_input_contracts::{InputEvent, InputOrigin};
use std::{collections::BTreeSet, path::Path};
/// 原始记录与派生记录引用验证，由独立日志进程独占。
pub struct RecordingWriter {
    journal: Journal,
    normalizer: Normalizer,
    raw: BTreeSet<RecordId>,
    pending: BTreeSet<RecordId>,
    phase: SessionPhase,
    temporal: super::association::TemporalIndex,
}
impl RecordingWriter {
    /// 创建新会话。调用者只有 Hook 就绪后才能报告 Ready。
    pub fn create(
        path: &Path,
        session: &Session,
        double_ms: u32,
        distance: i32,
    ) -> StorageResult<Self> {
        Ok(Self {
            journal: Journal::create(path, session)?,
            normalizer: Normalizer::new(session.qpc_frequency, double_ms, distance),
            raw: BTreeSet::new(),
            pending: BTreeSet::new(),
            phase: SessionPhase::Recording,
            temporal: super::association::TemporalIndex::new(session),
        })
    }
    /// 先返回已提交原始事实，再追加归一化操作；调用者在两者之间可发送锚定通知。
    pub fn raw(&mut self, event: InputEvent, qpc: i64) -> StorageResult<Record> {
        if self.phase != SessionPhase::Recording {
            return Err(StorageError::Format("当前状态不接受输入".into()));
        }
        if self.raw.len() >= 2_000_000 {
            return Err(StorageError::Format("会话原始记录达到200万条预算".into()));
        }
        let record = self.journal.append(RecordData::Raw(event), qpc)?;
        self.raw.insert(record.id);
        self.temporal.push(record.id, event.qpc);
        if requires_evidence(event) {
            self.pending.insert(record.id);
        }
        Ok(record)
    }
    /// 自身注入仍保留事实，但不产生递归操作。
    pub fn normalize(&mut self, record: &Record, qpc: i64) -> StorageResult<Option<Record>> {
        if let RecordData::Raw(event) = record.data {
            if event.origin == InputOrigin::ArgusFlow {
                return Ok(None);
            }
            if let Some(action) = self.normalizer.push(record.id, event) {
                return self
                    .journal
                    .append(RecordData::Interaction(action), qpc)
                    .map(Some);
            }
        }
        Ok(None)
    }
    /// 追加证据前验证它只引用已写入事实；状态水位由本写者维护。
    pub fn derived(&mut self, data: RecordData, qpc: i64) -> StorageResult<Record> {
        let refs = match &data {
            RecordData::Clipboard(s) => &s.raw,
            RecordData::Structure(s) => &s.raw,
            RecordData::Visual(s) => &s.raw,
            RecordData::Ocr(s) => &s.raw,
            RecordData::Attempt { raw, .. } => raw,
            _ => {
                return Err(StorageError::Format(
                    "父进程不能伪造原始记录、操作或状态".into(),
                ));
            }
        };
        let session_attempt = matches!(&data, RecordData::Attempt { raw, .. } if raw.is_empty());
        if (!session_attempt && refs.is_empty())
            || refs.len() > 2048
            || refs.iter().any(|id| !self.raw.contains(id))
        {
            return Err(StorageError::Format("派生引用未确认的原始记录".into()));
        }
        if !matches!(self.phase, SessionPhase::Recording | SessionPhase::Stopping) {
            return Err(StorageError::Format("派生结果晚于会话边界".into()));
        }
        let record = self.journal.append(data, qpc)?;
        if let RecordData::Attempt {
            stage: Stage::Derivation,
            raw,
            ..
        } = &record.data
        {
            for id in raw {
                self.pending.remove(id);
            }
        }
        Ok(record)
    }
    /// 状态边界前为所有未完成的来源追加取消结果，不让重新打开的记录永远待处理。
    pub fn finish_pending(&mut self, reason: &str, qpc: i64) -> StorageResult<Vec<Record>> {
        let pending: Vec<_> = self.pending.iter().copied().collect();
        let mut records = vec![];
        for raw in pending.chunks(2048) {
            records.push(self.journal.append(
                RecordData::Attempt {
                    raw: raw.to_vec(),
                    stage: Stage::Derivation,
                    outcome: Outcome::Cancelled,
                    reason: reason.into(),
                },
                qpc,
            )?);
        }
        self.pending.clear();
        Ok(records)
    }
    /// 视觉记录提交后，追加同一观察区间内的已提交输入引用。
    pub fn associate(&mut self, record: &Record, qpc: i64) -> StorageResult<Option<Record>> {
        self.temporal
            .associate(record)
            .map(|data| self.journal.append(data, qpc))
            .transpose()
    }
    /// 封闭状态转换；重复暂停／开始不是成功。
    pub fn transition(
        &mut self,
        phase: SessionPhase,
        reason: String,
        qpc: i64,
    ) -> StorageResult<Record> {
        let valid = matches!(
            (self.phase, phase),
            (
                SessionPhase::Recording,
                SessionPhase::Recording
                    | SessionPhase::Paused
                    | SessionPhase::Stopping
                    | SessionPhase::Faulted
            ) | (
                SessionPhase::Paused,
                SessionPhase::Recording | SessionPhase::Stopping | SessionPhase::Faulted
            ) | (
                SessionPhase::Stopping,
                SessionPhase::Stopped | SessionPhase::Faulted | SessionPhase::Interrupted
            )
        );
        if !valid {
            return Err(StorageError::Format("无效录制状态转换".into()));
        }
        let record = self
            .journal
            .append(RecordData::State { phase, reason }, qpc)?;
        self.phase = phase;
        self.normalizer.reset();
        self.temporal.reset();
        Ok(record)
    }
    /// 同步后追加可恢复水位；水位记录本身也同步。
    pub fn sync(&mut self, qpc: i64) -> StorageResult<(Record, u64)> {
        let through = self.journal.sync()?;
        let record = self
            .journal
            .append(RecordData::Durability { through }, qpc)?;
        let synced = self.journal.sync()?;
        Ok((record, synced))
    }
    /// 已写入与已同步。
    pub fn watermarks(&self) -> (u64, u64) {
        self.journal.watermarks()
    }
}
