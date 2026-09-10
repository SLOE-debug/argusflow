//! Windows UI Automation 与显式真实输入基础能力。

#![cfg(windows)]

mod application;
mod capture;
mod error;
mod input;
mod platform;
mod uia;
mod window;

pub use application::{Application, ApplicationOptions};
pub use argusflow_core::OperationOptions;
pub use capture::DxgiBackend;
pub use error::WindowsError;
pub use input::{InputAction, InputSequence, InputService, InputState};
pub use uia::{
    ControlType, ElementHandle, ElementSnapshot, Predicate, Query, ScrollAmount, SearchScope,
    SelectionAction, UiaAction, UiaConfig, UiaMatch, UiaRuntime, UiaState,
};
pub use window::{WindowIdentity, WindowInfo, WindowLocator};
