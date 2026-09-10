//! 调用者只接收普通数据及现有元素租约。
use super::super::worker::{Command, Response};
use crate::{ElementHandle, ElementSnapshot, UiaRuntime, WindowIdentity, WindowsError};
use argusflow_aql::{Attribute, BoundQuery, Value};
use argusflow_core::{FailureKind, Operation, ScreenPoint};
use std::collections::BTreeMap;

/// 一次 AQL 查询命中的 UIA 元素。
#[derive(Debug, Clone)]
pub struct UiaMatch {
    handle: ElementHandle,
    snapshot: ElementSnapshot,
    attributes: BTreeMap<Attribute, Value>,
}
impl UiaMatch {
    pub(crate) fn new(
        handle: ElementHandle,
        snapshot: ElementSnapshot,
        attributes: BTreeMap<Attribute, Value>,
    ) -> Self {
        Self {
            handle,
            snapshot,
            attributes,
        }
    }
    /// 有时限且绑定窗口的元素身份。
    pub fn handle(&self) -> &ElementHandle {
        &self.handle
    }
    /// 查询时的几何和基础属性。
    pub fn snapshot(&self) -> &ElementSnapshot {
        &self.snapshot
    }
    /// 查询要求的已知属性。
    pub fn attributes(&self) -> &BTreeMap<Attribute, Value> {
        &self.attributes
    }
}
impl UiaRuntime {
    /// 使用调用方的同一操作票据执行 AQL，不重置截止时间。
    pub async fn query_aql(
        &self,
        window: WindowIdentity,
        query: BoundQuery,
        operation: &Operation,
    ) -> Result<Vec<UiaMatch>, WindowsError> {
        match self
            .call_operation(Command::Aql(window, query), operation)
            .await?
        {
            Response::Aql(matches) => Ok(matches),
            _ => Err(protocol()),
        }
    }
    /// 复验元素租约、窗口归属和 UIA 命中位置，再交给真实输入服务。
    pub async fn aql_click_point(
        &self,
        handle: &ElementHandle,
        operation: &Operation,
    ) -> Result<ScreenPoint, WindowsError> {
        match self
            .call_operation(Command::ClickPoint(handle.clone()), operation)
            .await?
        {
            Response::Point(point) => Ok(point),
            _ => Err(protocol()),
        }
    }
    /// 聚焦保留控件原内容，不执行 SetValue 或全选。
    pub async fn focus_aql(
        &self,
        handle: &ElementHandle,
        operation: &Operation,
    ) -> Result<(), WindowsError> {
        match self
            .call_operation(Command::FocusAql(handle.clone()), operation)
            .await?
        {
            Response::Done => Ok(()),
            _ => Err(protocol()),
        }
    }
}
fn protocol() -> WindowsError {
    WindowsError::new(FailureKind::Protocol, "uia_aql", "UIA 返回的响应类型不一致")
}
