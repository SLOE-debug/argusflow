//! 紧凑时间线保留事件身份、采样范围与关联；大正文通过工具补读。
use super::*;
use serde_json::{Value, json};
impl Evidence {
    pub(crate) fn timeline(&self) -> Result<Value> {
        let mut rows = Vec::new();
        for record in self.records.values() {
            let data = match &record.data {
                RecordData::Interaction(v) => json!({"kind":"interaction","value":v}),
                RecordData::Raw(v) => json!({"kind":"raw","value":v}),
                RecordData::Structure(v) => {
                    json!({"kind":"structure","raw":v.raw,"source":v.source,"relation":v.relation,"identity":v.identity,"from_qpc":v.from_qpc,"through_qpc":v.through_qpc,"stale":v.stale,"truncated":v.truncated,"sensitive":v.sensitive,"target_confirmed":v.target_confirmed,"detail_tool":"inspect_evidence"})
                }
                RecordData::Clipboard(v) => {
                    json!({"kind":"clipboard","raw":v.raw,"detail_tool":"inspect_evidence"})
                }
                RecordData::Visual(v) => {
                    json!({"kind":"visual","raw":v.raw,"relation":v.relation,"version":v.version,"image_available":v.image.is_some(),"detail_tool":"view_change"})
                }
                RecordData::Ocr(v) => {
                    json!({"kind":"ocr","raw":v.raw,"blocks":v.blocks.len(),"detail_tool":"inspect_evidence"})
                }
                RecordData::Attempt {
                    raw,
                    stage,
                    outcome,
                    reason,
                } => {
                    json!({"kind":"attempt","raw":raw,"stage":stage,"outcome":outcome,"reason":reason})
                }
                _ => continue,
            };
            rows.push(json!({"id":record.id.to_string(),"written_qpc":record.written_qpc.to_string(),"data":data}));
        }
        let value =
            json!({"session":self.session,"timeline":rows,"required_ids":self.required_ids()});
        if serde_json::to_vec(&value)?.len() > 130000 {
            return Err(AiError::Budget(
                "时间线超过 130 KiB，未截断事实；请使用更短的录制".into(),
            ));
        }
        Ok(value)
    }
}
