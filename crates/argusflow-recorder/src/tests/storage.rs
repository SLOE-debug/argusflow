use super::fixtures::{raw, text};
use crate::*;

#[tokio::test]
async fn raw_and_ai_semantics_are_separate_and_manifest_is_complete() {
    let id = uuid::Uuid::new_v4();
    let root = std::env::temp_dir().join(format!("argusflow-recorder-test-{id}"));
    let raw = RawTrace {
        events: vec![raw(1, text("fixture"))],
    };
    let normalized = TraceNormalizer::normalize(&raw);
    let trace = RecordingTrace {
        schema_version: 1,
        recording_id: id,
        started_at_unix_ms: 123,
        raw,
        normalized,
        dropped_events: 0,
    };
    let files = crate::storage::save(&root, &trace).await.unwrap();
    let raw: RawTrace =
        serde_json::from_slice(&tokio::fs::read(&files.raw).await.unwrap()).unwrap();
    let semantic: NormalizedSemanticTrace =
        serde_json::from_slice(&tokio::fs::read(&files.normalized).await.unwrap()).unwrap();
    assert_eq!(raw, trace.raw);
    assert_eq!(semantic, trace.normalized);
    assert!(
        !serde_json::to_value(&semantic)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("raw")
    );
    assert!(tokio::fs::try_exists(&files.manifest).await.unwrap());
    // 保存重试不得改变语义；Windows rename 的目标替换行为也由此验证。
    crate::storage::save(&root, &trace).await.unwrap();
    let summaries = crate::history::list(&root).await.unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].semantic_record_count,
        trace.normalized.records.len()
    );
    let loaded = crate::history::load(&root, id).await.unwrap();
    assert_eq!(loaded.trace.raw, trace.raw);
    assert_eq!(loaded.trace.normalized, trace.normalized);
    for file in [&files.raw, &files.normalized, &files.manifest] {
        tokio::fs::remove_file(file).await.unwrap();
    }
    tokio::fs::remove_dir(root.join(id.to_string()))
        .await
        .unwrap();
    tokio::fs::remove_dir(root).await.unwrap();
}
