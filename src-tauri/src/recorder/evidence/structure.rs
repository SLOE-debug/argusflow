//! 结构观察记录真实请求区间；不伪装操作前快照。
use super::{attempt, options, services::EvidenceServices};
use argusflow_input_contracts::{InputEvent, InputKind};
use argusflow_recorder::*;
use argusflow_windows::{UiaObservedNode, listening::qpc};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;
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
        let from = qpc();
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
                let target = observed.target;
                let structure=Structure{raw:raw.to_vec(),source:StructureSource::Uia,relation:Relation::After,from_qpc:from,through_qpc:qpc(),identity:BTreeMap::from([("hwnd_hint".into(),event.window.handle.to_string()),("pid".into(),target.pid.to_string()),("foreground_epoch".into(),event.window.epoch.to_string())]),properties:properties(&target),ancestors:observed.ancestors.iter().map(properties).collect(),bounds:Some(target.bounds),truncated:observed.truncated,stale:target.pid as u32!=event.window.pid,sensitive:target.password,target_confirmed:false,scope:"事件后定点或焦点观察；不证明目标在输入瞬间相同；祖先最多4层；窗口线索不保证复用安全".into()};
                let _ = tx
                    .send(RecorderCommand::evidence(RecordData::Structure(
                        structure.clone(),
                    )))
                    .await;
                Some(structure)
            }
            Err(error) => {
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
