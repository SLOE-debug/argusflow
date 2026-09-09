//! 每个模型档位和设备同时只持有一个原生引擎，Clone 复用它。
use crate::OcrError as Failure;
use crate::{Device, ModelTier, OcrConfig};
use argusflow_core::FailureKind;
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};
type Key = (ModelTier, Device);
static ENGINES: OnceLock<Mutex<HashSet<Key>>> = OnceLock::new();
pub(crate) struct Owner(Key);
impl Owner {
    pub(crate) fn acquire(config: &OcrConfig) -> Result<Self, Failure> {
        let key = (config.tier, config.device);
        if !ENGINES
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(key)
        {
            return Err(Failure::new(
                FailureKind::Busy,
                "ocr_load",
                "该档位和设备已有引擎或尚未退出的原生调用，请复用或等待 shutdown",
            ));
        }
        Ok(Self(key))
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Some(engines) = ENGINES.get() {
            engines
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&self.0);
        }
    }
}
