//! 以视觉事实驱动的安全滚动会话。

// 滚动闭环的领域模型已经独立完成，但当前执行链只需要把强类型 wheel
// 步数交给 Windows 输入层；其余实现待 runtime 滚动编排接入后再进入生产调用图。
#![allow(dead_code)]

mod controller;
mod displacement;
mod end;
mod history;
mod model;
mod session;

pub use model::WheelSteps;
