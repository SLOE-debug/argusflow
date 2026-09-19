//! OCR 采样、坐标确认与输入动作。
use super::{SourceBackend, contract::SourceFuture, input::click};
use argusflow_aql::BoundQuery;
use argusflow_capture_contracts::{PixelRect, SourceId};
use argusflow_core::{Failure, FailureKind, Operation, ScreenPoint};
use argusflow_vision::{OcrMatch, SampledOcr, SampledOcrError, SampledOcrResult};
use argusflow_windows::{InputAction, InputSequence, InputService, WindowIdentity};
pub(crate) struct OcrSource {
    pub sampler: SampledOcr,
    pub source: SourceId,
    pub region: PixelRect,
    pub window: WindowIdentity,
    pub input: InputService,
}
impl OcrSource {
    async fn sample(
        &self,
        query: &BoundQuery,
        operation: &Operation,
    ) -> Result<SampledOcrResult, Failure> {
        let mut sequence = self.input.sequence(operation)?;
        sequence
            .perform(self.window.clone(), InputAction::ActivateWindow)
            .await?;
        let surfaces = self.window.physical_surfaces()?;
        let result = self
            .sampler
            .sample_aql(self.source, self.region, query, operation)
            .await
            .map_err(ocr_error)?;
        let bounds = result.bounds();
        let outside = surfaces.iter().any(|r| {
            r.x() < bounds.x()
                || r.y() < bounds.y()
                || i64::from(r.x()) + i64::from(r.width())
                    > i64::from(bounds.x()) + i64::from(bounds.width())
                || i64::from(r.y()) + i64::from(r.height())
                    > i64::from(bounds.y()) + i64::from(bounds.height())
        });
        self.window.require_foreground()?;
        if outside || self.window.physical_surfaces()? != surfaces {
            return Err(Failure::new(
                FailureKind::StaleHandle,
                "ocr_window_surfaces",
                "识别期间窗口或菜单范围已改变",
            ));
        }
        Ok(result.restricted_to(&surfaces))
    }
    async fn click_in(
        &self,
        target: &(SampledOcrResult, OcrMatch),
        operation: &Operation,
        sequence: &mut InputSequence<'_>,
    ) -> Result<(), Failure> {
        sequence
            .perform(self.window.clone(), InputAction::ActivateWindow)
            .await?;
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
    fn preview<'a>(
        &'a self,
        query: &'a BoundQuery,
        operation: &'a Operation,
    ) -> SourceFuture<'a, Vec<argusflow_aql::SpatialPreview>> {
        Box::pin(async move {
            self.sample(query, operation)
                .await?
                .result()
                .preview_aql(query, operation)
        })
    }
    fn find<'a>(
        &'a self,
        query: &'a BoundQuery,
        operation: &'a Operation,
    ) -> SourceFuture<'a, Vec<Self::Target>> {
        Box::pin(async move {
            let sample = self.sample(query, operation).await?;
            let targets = sample.result().query_aql(query, operation)?;
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
fn ocr_error(error: SampledOcrError) -> Failure {
    match error {
        SampledOcrError::Capture(argusflow_capture_contracts::CaptureError::Failure(error)) => {
            error
        }
        SampledOcrError::Ocr(error) => error.into(),
    }
}
