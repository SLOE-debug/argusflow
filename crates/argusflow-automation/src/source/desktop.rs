//! 桌面输入编排，OCR 不静态依赖 Windows 实现。
use super::{SourceBackend, contract::SourceFuture};
use argusflow_aql::{Attribute, BoundQuery, Value};
use argusflow_capture_contracts::{PixelRect, SourceId};
use argusflow_core::{ClickCount, Failure, FailureKind, MouseButton, Operation, ScreenPoint};
use argusflow_vision::{OcrMatch, SampledOcr, SampledOcrError, SampledOcrResult};
use argusflow_windows::{
    InputAction, InputSequence, InputService, UiaMatch, UiaRuntime, WindowIdentity,
};

pub(crate) struct UiaSource {
    pub runtime: UiaRuntime,
    pub window: WindowIdentity,
    pub input: InputService,
}
impl SourceBackend for UiaSource {
    type Target = UiaMatch;
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
pub(crate) struct OcrSource {
    pub sampler: SampledOcr,
    pub source: SourceId,
    pub region: PixelRect,
    pub window: WindowIdentity,
    pub input: InputService,
}
impl OcrSource {
    async fn click_in(
        &self,
        target: &(SampledOcrResult, OcrMatch),
        operation: &Operation,
        sequence: &mut InputSequence<'_>,
    ) -> Result<(), Failure> {
        self.window.require_foreground()?;
        self.sampler
            .confirm_aql_result(self.source, &target.0, operation)
            .await
            .map_err(ocr_error)?;
        let polygon = target.0.screen_polygon(target.1.index()).ok_or_else(|| {
            Failure::new(
                FailureKind::StaleHandle,
                "aql_ocr_position",
                "OCR 文字不属于当前采样",
            )
        })?;
        let point = ScreenPoint {
            x: (polygon.iter().map(|p| i64::from(p.x)).sum::<i64>() / 4) as i32,
            y: (polygon.iter().map(|p| i64::from(p.y)).sum::<i64>() / 4) as i32,
        };
        click(sequence, &self.window, point).await
    }
}
impl SourceBackend for OcrSource {
    type Target = (SampledOcrResult, OcrMatch);
    fn find<'a>(
        &'a self,
        query: &'a BoundQuery,
        operation: &'a Operation,
    ) -> SourceFuture<'a, Vec<Self::Target>> {
        Box::pin(async move {
            self.window.validate()?;
            let (sample, targets) = self
                .sampler
                .query_aql(self.source, self.region, query, operation)
                .await
                .map_err(ocr_error)?;
            self.window.validate()?;
            Ok(targets
                .into_iter()
                .map(|target| (sample.clone(), target))
                .collect())
        })
    }
    fn click<'a>(
        &'a self,
        target: &'a Self::Target,
        operation: &'a Operation,
    ) -> SourceFuture<'a, ()> {
        Box::pin(async move {
            let mut sequence = self.input.sequence(operation)?;
            self.click_in(target, operation, &mut sequence).await
        })
    }
    fn type_text<'a>(
        &'a self,
        target: &'a Self::Target,
        text: &'a str,
        operation: &'a Operation,
    ) -> SourceFuture<'a, ()> {
        Box::pin(async move {
            let mut sequence = self.input.sequence(operation)?;
            self.click_in(target, operation, &mut sequence).await?;
            sequence
                .perform(self.window.clone(), InputAction::Text(text.into()))
                .await
                .map_err(Into::into)
        })
    }
}
async fn click(
    input: &mut InputSequence<'_>,
    window: &WindowIdentity,
    point: ScreenPoint,
) -> Result<(), Failure> {
    input
        .perform(
            window.clone(),
            InputAction::Click {
                point,
                button: MouseButton::Left,
                count: ClickCount::Single,
            },
        )
        .await
        .map_err(Into::into)
}
fn ocr_error(error: SampledOcrError) -> Failure {
    match error {
        SampledOcrError::Capture(argusflow_capture_contracts::CaptureError::Failure(error)) => {
            error
        }
        SampledOcrError::Capture(error) => {
            Failure::new(error.kind(), "aql_ocr", "采样历史不完整").with_source(error)
        }
        SampledOcrError::Ocr(error) => error.into(),
    }
}
