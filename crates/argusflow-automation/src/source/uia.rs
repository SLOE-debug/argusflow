//! UIA 目标的动作与焦点确认。
use super::{
    SourceBackend,
    contract::{FocusedInput, SourceFuture},
    input::click,
};
use argusflow_aql::{Attribute, BoundQuery, Value};
use argusflow_core::{Failure, FailureKind, Operation};
use argusflow_windows::{InputAction, InputService, UiaMatch, UiaRuntime, WindowIdentity};
pub(crate) struct UiaSource {
    pub runtime: UiaRuntime,
    pub window: WindowIdentity,
    pub input: InputService,
}
impl SourceBackend for UiaSource {
    type Target = UiaMatch;
    fn preview<'a>(
        &'a self,
        query: &'a BoundQuery,
        operation: &'a Operation,
    ) -> SourceFuture<'a, Vec<argusflow_aql::SpatialPreview>> {
        Box::pin(async move {
            self.runtime
                .preview_aql(self.window.clone(), query.clone(), operation)
                .await
                .map_err(Into::into)
        })
    }
    fn focused_input<'a>(
        &'a self,
        target: &'a UiaMatch,
        input: FocusedInput<'a>,
        operation: &'a Operation,
    ) -> SourceFuture<'a, ()> {
        Box::pin(async move {
            let mut sequence = self.input.sequence(operation)?;
            self.window.require_foreground()?;
            self.runtime
                .confirm_aql_focus(target.handle(), operation)
                .await?;
            let action = match input {
                FocusedInput::Text(text) => InputAction::Text(text.into()),
                FocusedInput::Keys(keys) => InputAction::Chord(keys.to_vec()),
            };
            sequence
                .perform(self.window.clone(), action)
                .await
                .map_err(Into::into)
        })
    }
    fn find<'a>(
        &'a self,
        query: &'a BoundQuery,
        operation: &'a Operation,
    ) -> SourceFuture<'a, Vec<UiaMatch>> {
        Box::pin(async move {
            self.runtime
                .query_aql(self.window.clone(), query.clone(), operation)
                .await
                .map_err(Into::into)
        })
    }
    fn click<'a>(&'a self, target: &'a UiaMatch, operation: &'a Operation) -> SourceFuture<'a, ()> {
        Box::pin(async move {
            let mut sequence = self.input.sequence(operation)?;
            self.window.require_foreground()?;
            let point = self
                .runtime
                .aql_click_point(target.handle(), operation)
                .await?;
            click(&mut sequence, &self.window, point).await
        })
    }
    fn type_text<'a>(
        &'a self,
        target: &'a UiaMatch,
        text: &'a str,
        operation: &'a Operation,
    ) -> SourceFuture<'a, ()> {
        Box::pin(async move {
            let mut sequence = self.input.sequence(operation)?;
            self.window.require_foreground()?;
            if target.snapshot().control_type != 50004
                || target.attributes().get(&Attribute::Enabled) != Some(&Value::Boolean(true))
            {
                return Err(Failure::new(
                    FailureKind::Unsupported,
                    "aql_type_text",
                    "目标不是可用的 UIA 输入框",
                ));
            }
            self.runtime.focus_aql(target.handle(), operation).await?;
            sequence
                .perform(self.window.clone(), InputAction::Text(text.into()))
                .await
                .map_err(Into::into)
        })
    }
}
