//! 桌面能力装配、有序日志和单次运行生命周期。
mod assembly;
mod bundle;
mod journal;
mod manager;
mod messages;
pub(crate) use bundle::load_bundle;
pub(crate) use manager::RunManager;
pub(crate) use messages::RunMessage;
