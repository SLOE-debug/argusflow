//! 结构观察记录真实请求区间；不伪装操作前快照。
use super::{attempt, options, services::EvidenceServices};
use argusflow_input_contracts::{InputEvent, InputKind};
use argusflow_recorder::*;
use argusflow_windows::{UiaObservedNode, listening::qpc};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;
/// 仅保留上一份有界快照；窗口代际变化时清除，不跨窗口比较差异。
pub(super) struct PreviousObservation {
    raw: u64,
    window: argusflow_input_contracts::WindowContext,
    value: argusflow_windows::UiaObservation,
}
impl EvidenceServices {
    pub(super) async fn structure(
        &self,
        raw: &[u64],
        event: InputEvent,
        tx: &mpsc::Sender<RecorderCommand>,
    ) -> Option<Structure> {
        if !self.observing.load(Ordering::Acquire) {
            return None;
        }
        // 串行化同一录制的结构快照，避免后完成的旧请求覆盖新基线。
        let mut previous = self.previous.lock().await;
        if !self.observing.load(Ordering::Acquire) {
            return None;
        }
        let from = qpc();
        if previous.as_ref().is_some_and(|p| p.window != event.window) {
            *previous = None;
        }
        let point = if matches!(event.kind, InputKind::Key { .. } | InputKind::Context) {
            None
        } else {
            Some([event.point.x, event.point.y])
        };
        let result = match &self.uia {
            Some(uia) => uia
                .observe_target(point, options(350))
                .await
                .map_err(|e| e.to_string()),
            None => Err("共享 UIA 服务不可用".into()),
        };
        match result {
            Ok(observed) => {
                if !self.observing.load(Ordering::Acquire) {
                    return None;
                }
                let target = &observed.target;
                let mut attributes = properties(target);
                let change = observed
                    .text_change_from(previous.as_ref().map(|observation| &observation.value));
                let encoded = serde_json::to_value((&observed.text, &change))
                    .and_then(serde_json::from_value::<[Property; 2]>);
                let Ok([text, change]) = encoded else {
                    *previous = None;
                    let _ = tx
                        .send(RecorderCommand::evidence(attempt(
                            raw,
                            Stage::Uia,
                            Outcome::Failed,
                            "文本观察序列化失败",
                        )))
                        .await;
                    return None;
                };
                attributes.insert("text_change".into(), change);
                attributes.insert("text".into(), text);
                attributes.insert(
                    "runtime_id".into(),
                    Property::List(
                        observed
                            .runtime_id
                            .iter()
                            .map(|v| Property::Number(f64::from(*v)))
                            .collect(),
                    ),
                );
                if let Some(previous) = previous.as_ref() {
                    attributes.insert(
                        "previous_observation_raw".into(),
                        Property::Number(previous.raw as f64),
                    );
                }
                let text_truncated = matches!(&observed.text,argusflow_windows::UiaTextObservation::Available(t) if t.truncated || t.document_truncated);
                let structure=Structure{raw:raw.to_vec(),source:StructureSource::Uia,relation:Relation::After,from_qpc:from,through_qpc:qpc(),identity:BTreeMap::from([("hwnd_hint".into(),event.window.handle.to_string()),("pid".into(),target.pid.to_string()),("foreground_epoch".into(),event.window.epoch.to_string())]),properties:attributes,ancestors:observed.ancestors.iter().map(properties).collect(),bounds:Some(target.bounds),truncated:observed.truncated || text_truncated,stale:target.pid as u32!=event.window.pid,sensitive:target.password,target_confirmed:false,scope:"事件后定点或焦点只读观察；不证明输入瞬间状态；选区偏移使用提供者UTF-16文本；变化仅比较两次采样，不声称覆盖中间所有变化".into()};
                if structure.sensitive || structure.stale {
                    *previous = None;
                } else if let Some(id) = raw.first() {
                    *previous = Some(PreviousObservation {
                        raw: *id,
                        window: event.window,
                        value: observed,
                    });
                }
                let _ = tx
                    .send(RecorderCommand::evidence(RecordData::Structure(
                        structure.clone(),
                    )))
                    .await;
                Some(structure)
            }
            Err(error) => {
                *previous = None;
                let _ = tx
                    .send(RecorderCommand::evidence(attempt(
                        raw,
                        Stage::Uia,
                        Outcome::Unavailable,
                        &error,
                    )))
                    .await;
                None
            }
        }
    }
}

fn properties(node: &UiaObservedNode) -> BTreeMap<String, Property> {
    let mut result = BTreeMap::from([
        ("name".into(), Property::Text(node.name.clone())),
        (
            "automation_id".into(),
            Property::Text(node.automation_id.clone()),
        ),
        ("class_name".into(), Property::Text(node.class_name.clone())),
        ("role".into(), Property::Number(f64::from(node.role))),
        ("enabled".into(), Property::Bool(node.enabled)),
        ("focused".into(), Property::Bool(node.focused)),
        ("password".into(), Property::Bool(node.password)),
    ]);
    if let Some(value) = &node.value {
        result.insert("observed_value".into(), Property::Text(value.clone()));
    }
    result
}
