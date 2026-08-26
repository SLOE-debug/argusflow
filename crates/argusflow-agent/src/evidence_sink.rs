//! Failure Evidence 的宿主持久化实现与不可变计划配置。

use std::{
    fmt, fs,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use argusflow_core::BackendKind;
use argusflow_query::BranchPath;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    EvidenceArtifactData, EvidenceArtifactKind, EvidenceBudget, EvidenceCapturePolicy,
    EvidenceOutcome, EvidenceRecord, EvidenceRetentionPolicy, EvidenceTrigger,
};

/// 持久化后的稳定引用；ExecutionEvent 只应传播这个小对象的字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceReference {
    /// 宿主生成的 evidence 唯一标识。
    pub evidence_id: Uuid,
    /// 可选的宿主相对或绝对位置；远端 sink 可以不提供。
    pub location: Option<PathBuf>,
}

/// Evidence sink 的结构化持久化错误。
#[derive(Debug, Error)]
pub enum EvidenceSinkError {
    /// artifact 路径不是安全相对路径。
    #[error("evidence artifact path must be a normalized relative path: {path}")]
    InvalidArtifactPath {
        /// 被拒绝的原始路径。
        path: PathBuf,
    },
    /// bundle 超过宿主大小限制。
    #[error("evidence bundle exceeds byte limit {limit}")]
    ByteLimitExceeded {
        /// 生效的总字节限制。
        limit: usize,
    },
    /// JSON 序列化失败。
    #[error("failed to serialize evidence manifest: {source}")]
    Serialize {
        /// serde 返回的具体错误。
        #[source]
        source: serde_json::Error,
    },
    /// 文件系统持久化失败。
    #[error("failed to persist evidence: {source}")]
    Io {
        /// 标准文件系统错误。
        #[source]
        source: std::io::Error,
    },
}

/// Runtime/host 注入的 artifact 持久化边界。
#[async_trait]
pub trait EvidenceSink: fmt::Debug + Send + Sync {
    /// 持久化一次已完成采集的记录并返回稳定引用。
    async fn persist(&self, record: EvidenceRecord)
    -> Result<EvidenceReference, EvidenceSinkError>;
}

/// 丢弃所有记录的默认 sink；与 `Off` 策略组合时不会产生外部副作用。
#[derive(Debug, Default)]
pub struct DiscardEvidenceSink;

#[async_trait]
impl EvidenceSink for DiscardEvidenceSink {
    async fn persist(
        &self,
        _record: EvidenceRecord,
    ) -> Result<EvidenceReference, EvidenceSinkError> {
        Ok(EvidenceReference {
            evidence_id: Uuid::new_v4(),
            location: None,
        })
    }
}

/// 测试和嵌入式宿主可读取的内存 sink。
#[derive(Debug, Default)]
pub struct InMemoryEvidenceSink {
    /// 按持久化顺序保存的不可变记录副本。
    records: Mutex<Vec<EvidenceRecord>>,
}

impl InMemoryEvidenceSink {
    /// 返回当前全部记录的快照。
    pub fn records(&self) -> Vec<EvidenceRecord> {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

#[async_trait]
impl EvidenceSink for InMemoryEvidenceSink {
    async fn persist(
        &self,
        record: EvidenceRecord,
    ) -> Result<EvidenceReference, EvidenceSinkError> {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(record);
        Ok(EvidenceReference {
            evidence_id: Uuid::new_v4(),
            location: None,
        })
    }
}

/// 把 bundle 按 attempt 目录持久化为 manifest 与独立 artifact 的文件 sink。
#[derive(Debug)]
pub struct FileSystemEvidenceSink {
    /// 每个 evidence id 目录的父目录。
    root: PathBuf,
}

impl FileSystemEvidenceSink {
    /// 创建不触碰文件系统的 sink；目录只在第一次持久化时创建。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[async_trait]
impl EvidenceSink for FileSystemEvidenceSink {
    async fn persist(
        &self,
        record: EvidenceRecord,
    ) -> Result<EvidenceReference, EvidenceSinkError> {
        let evidence_id = Uuid::new_v4();
        let attempt_directory = self.root.join(evidence_id.to_string());
        let mut prepared_artifacts = Vec::new();
        let mut total_bytes = 0_usize;
        for artifact in &record.bundle.artifacts {
            if artifact.kind == EvidenceArtifactKind::Screenshot
                && !record.retention.persist_screenshot
            {
                continue;
            }
            validate_relative_path(&artifact.relative_path)?;
            let bytes = artifact_bytes(&artifact.data)?;
            total_bytes = total_bytes.saturating_add(bytes.len());
            if total_bytes > record.retention.max_total_bytes {
                return Err(EvidenceSinkError::ByteLimitExceeded {
                    limit: record.retention.max_total_bytes,
                });
            }
            prepared_artifacts.push(PreparedArtifact {
                destination: attempt_directory.join(&artifact.relative_path),
                bytes,
                manifest: EvidenceArtifactManifest {
                    kind: artifact.kind,
                    path: artifact.relative_path.clone(),
                    sensitive: artifact.sensitive,
                },
            });
        }

        fs::create_dir_all(&attempt_directory)
            .map_err(|source| EvidenceSinkError::Io { source })?;
        for artifact in &prepared_artifacts {
            if let Some(parent) = artifact.destination.parent() {
                fs::create_dir_all(parent).map_err(|source| EvidenceSinkError::Io { source })?;
            }
            fs::write(&artifact.destination, &artifact.bytes)
                .map_err(|source| EvidenceSinkError::Io { source })?;
        }

        let manifest = EvidenceManifest {
            schema_version: record.bundle.schema_version,
            evidence_id,
            backend: record.bundle.backend,
            branch_path: record.bundle.branch_path,
            trigger: record.bundle.trigger,
            query: record.bundle.query,
            outcome: record.outcome,
            artifacts: prepared_artifacts
                .into_iter()
                .map(|artifact| artifact.manifest)
                .collect(),
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|source| EvidenceSinkError::Serialize { source })?;
        fs::write(attempt_directory.join("manifest.json"), manifest_bytes)
            .map_err(|source| EvidenceSinkError::Io { source })?;

        Ok(EvidenceReference {
            evidence_id,
            location: Some(attempt_directory),
        })
    }
}

/// 完成路径和大小校验、尚未写入的 artifact。
struct PreparedArtifact {
    /// attempt 目录内的最终路径。
    destination: PathBuf,
    /// 已序列化内容。
    bytes: Vec<u8>,
    /// manifest 索引项。
    manifest: EvidenceArtifactManifest,
}

/// FileSystemEvidenceSink 写入的稳定 manifest。
#[derive(Serialize)]
struct EvidenceManifest {
    /// schema 演进版本。
    schema_version: u16,
    /// 本次 evidence 唯一标识。
    evidence_id: Uuid,
    /// 失败后端。
    backend: BackendKind,
    /// 失败分支。
    branch_path: BranchPath,
    /// 采集触发器。
    trigger: EvidenceTrigger,
    /// 规范化查询。
    query: String,
    /// 失败是否由 fallback 恢复。
    outcome: EvidenceOutcome,
    /// 实际写入的 artifacts。
    artifacts: Vec<EvidenceArtifactManifest>,
}

/// manifest 中不携带 artifact bytes 的只读索引项。
#[derive(Serialize)]
struct EvidenceArtifactManifest {
    /// artifact 类别。
    kind: EvidenceArtifactKind,
    /// 相对 attempt 目录的路径。
    path: PathBuf,
    /// 是否包含敏感现场。
    sensitive: bool,
}

/// 把 artifact 内容转换为可直接落盘的字节。
fn artifact_bytes(data: &EvidenceArtifactData) -> Result<Vec<u8>, EvidenceSinkError> {
    match data {
        EvidenceArtifactData::Text(text) => Ok(text.as_bytes().to_vec()),
        EvidenceArtifactData::Json(value) => serde_json::to_vec_pretty(value)
            .map_err(|source| EvidenceSinkError::Serialize { source }),
        EvidenceArtifactData::Binary(bytes) => Ok(bytes.clone()),
    }
}

/// 拒绝绝对路径、父级跳转和平台前缀，确保 artifact 不能逃出 attempt 目录。
fn validate_relative_path(path: &Path) -> Result<(), EvidenceSinkError> {
    let valid = !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if valid {
        Ok(())
    } else {
        Err(EvidenceSinkError::InvalidArtifactPath {
            path: path.to_path_buf(),
        })
    }
}

/// PreparedPlan 使用的不可变证据策略与宿主依赖。
#[derive(Clone)]
pub struct EvidenceSettings {
    /// 捕获时机。
    pub policy: EvidenceCapturePolicy,
    /// 采集硬预算。
    pub budget: EvidenceBudget,
    /// 敏感数据与持久化约束。
    pub retention: EvidenceRetentionPolicy,
    /// 计划外部的持久化实现。
    pub sink: Arc<dyn EvidenceSink>,
}

impl Default for EvidenceSettings {
    fn default() -> Self {
        Self {
            policy: EvidenceCapturePolicy::Off,
            budget: EvidenceBudget::default(),
            retention: EvidenceRetentionPolicy::default(),
            sink: Arc::new(DiscardEvidenceSink),
        }
    }
}

impl fmt::Debug for EvidenceSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvidenceSettings")
            .field("policy", &self.policy)
            .field("budget", &self.budget)
            .field("retention", &self.retention)
            .field("sink", &self.sink)
            .finish()
    }
}
