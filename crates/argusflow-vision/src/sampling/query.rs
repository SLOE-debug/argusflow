//! 查询调用共享采样服务，但调用者自己的截止时间和取消始终生效。
use crate::{OcrMatch, SampledOcr, SampledOcrError, SampledOcrResult, ocr_query_capabilities};
use argusflow_aql::BoundQuery;
use argusflow_capture_contracts::{
    CaptureError, PixelRect, SampleContent, SampleRequest, SourceId,
};
use argusflow_core::{FailureKind, Operation, OperationOptions};
use std::time::Duration;

impl SampledOcr {
    /// 稳定采样、OCR 和筛选受同一调用票据约束；取消会释放共享识别等待者。
    pub async fn query_aql(
        &self,
        source: SourceId,
        region: PixelRect,
        query: &BoundQuery,
        operation: &Operation,
    ) -> Result<(SampledOcrResult, Vec<OcrMatch>), SampledOcrError> {
        let result = self.sample_aql(source, region, query, operation).await?;
        let matches = result
            .result()
            .query_aql(query, operation)
            .map_err(CaptureError::from)?;
        Ok((result, matches))
    }
    /// 使用同一稳定采样直接生成预览，不先执行并丢弃一次查询。
    pub async fn preview_aql(
        &self,
        source: SourceId,
        region: PixelRect,
        query: &BoundQuery,
        operation: &Operation,
    ) -> Result<Vec<argusflow_aql::SpatialPreview>, SampledOcrError> {
        let result = self.sample_aql(source, region, query, operation).await?;
        result
            .result()
            .preview_aql(query, operation)
            .map_err(|e| SampledOcrError::Capture(CaptureError::from(e)))
    }
    /// 返回通过能力检查和总时限约束的采样，供组合区域先筛选再执行 AQL。
    pub async fn sample_aql(
        &self,
        source: SourceId,
        region: PixelRect,
        query: &BoundQuery,
        operation: &Operation,
    ) -> Result<SampledOcrResult, SampledOcrError> {
        ocr_query_capabilities()
            .check(query)
            .map_err(CaptureError::from)?;
        operation
            .check("ocr_aql_prepare")
            .map_err(CaptureError::from)?;
        let mut guard = operation.cancel_on_drop();
        // 单次采样保持有限预算；流程根票据可以没有截止时间。
        let options = OperationOptions::new(operation.remaining().min(Duration::from_secs(86_400)))
            .map_err(CaptureError::from)?;
        let recognize = self.recognize(source, region, options);
        tokio::pin!(recognize);
        let result = loop {
            tokio::select! {
                result = &mut recognize => break result?,
                _ = tokio::time::sleep(operation.remaining().min(Duration::from_millis(8))) => operation.check("ocr_aql_wait").map_err(CaptureError::from)?,
            }
        };
        operation
            .check("ocr_aql_complete")
            .map_err(CaptureError::from)?;
        guard.disarm();
        Ok(result)
    }
    /// 点击之前精确比较识别区域；画面变化时返回失效，不使用旧坐标。
    pub async fn confirm_aql_result(
        &self,
        source: SourceId,
        result: &SampledOcrResult,
        operation: &Operation,
    ) -> Result<(), SampledOcrError> {
        let sampling = self.inner.source.sample(
            SampleRequest {
                source,
                region: result.token.region,
                quiet: Duration::ZERO,
                previous: Some(result.token.clone()),
            },
            operation.clone(),
        );
        tokio::pin!(sampling);
        let sample = loop {
            tokio::select! {
                result = &mut sampling => break result?,
                _ = tokio::time::sleep(operation.remaining().min(Duration::from_millis(8))) => operation.check("ocr_aql_confirm").map_err(CaptureError::from)?,
            }
        };
        if !matches!(sample.content, SampleContent::Unchanged)
            || !sample.token.snapshot.validity.valid()
        {
            return Err(CaptureError::new(
                FailureKind::StaleHandle,
                "ocr_aql_confirm",
                "OCR 识别后目标区域已经改变",
            )
            .into());
        }
        operation
            .check("ocr_aql_confirm")
            .map_err(CaptureError::from)?;
        Ok(())
    }
}
