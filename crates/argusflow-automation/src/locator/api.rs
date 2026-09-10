//! 调用者绑定来源和冻结查询，通过同一 Locator 发起操作。
use super::execution::{Action, execute};
use crate::source::BrowserSource;
#[cfg(windows)]
use crate::source::{OcrSource, UiaSource};
use crate::{LocatedElement, QuerySource};
use argusflow_aql::{AqlError, Bindings, BoundQuery, CompiledQuery};
use argusflow_core::{Failure, Operation, OperationOptions};

/// 显式来源与不可变查询的组合；Clone 不创建平台资源。
#[derive(Clone)]
pub struct Locator {
    source: QuerySource,
    query: BoundQuery,
}
impl Locator {
    /// 创建参数已冻结的定位器。
    pub fn new(source: QuerySource, query: BoundQuery) -> Self {
        Self { source, query }
    }
    /// 将编译好的查询与类型化参数绑定。
    pub fn bind(
        source: QuerySource,
        query: &CompiledQuery,
        bindings: &Bindings,
    ) -> Result<Self, AqlError> {
        Ok(Self::new(source, query.bind(bindings)?))
    }
    /// 返回完整有序结果；预算耗尽报错。
    pub async fn find_all(
        &self,
        options: OperationOptions,
    ) -> Result<Vec<LocatedElement>, Failure> {
        self.find_all_with_operation(&Operation::new(options)).await
    }
    /// 沿用调用方票据定位，支持外部取消。
    pub async fn find_all_with_operation(
        &self,
        operation: &Operation,
    ) -> Result<Vec<LocatedElement>, Failure> {
        self.run(Action::FindAll, operation).await
    }
    /// 查询必须恰好一个结果。
    pub async fn find_unique(&self, options: OperationOptions) -> Result<LocatedElement, Failure> {
        self.find_unique_with_operation(&Operation::new(options))
            .await
    }
    /// 沿用调用方票据并要求唯一结果。
    pub async fn find_unique_with_operation(
        &self,
        operation: &Operation,
    ) -> Result<LocatedElement, Failure> {
        let mut results = self.run(Action::FindUnique, operation).await?;
        Ok(results.remove(0))
    }
    /// 重新定位、验证后左键单击一次；不会自动激活窗口。
    pub async fn click(&self, options: OperationOptions) -> Result<(), Failure> {
        self.click_with_operation(&Operation::new(options)).await
    }
    /// 使用调用方已有的截止时间和取消票据执行点击。
    pub async fn click_with_operation(&self, operation: &Operation) -> Result<(), Failure> {
        self.run(Action::Click, operation).await.map(|_| ())
    }
    /// 重新定位、聚焦后在光标处插入；OCR 通过点击目标建立焦点。
    pub async fn type_text(&self, text: &str, options: OperationOptions) -> Result<(), Failure> {
        self.type_text_with_operation(text, &Operation::new(options))
            .await
    }
    /// 保留同一请求时限与副作用状态的输入入口。
    pub async fn type_text_with_operation(
        &self,
        text: &str,
        operation: &Operation,
    ) -> Result<(), Failure> {
        self.run(Action::TypeText(text), operation)
            .await
            .map(|_| ())
    }
    async fn run(
        &self,
        action: Action<'_>,
        operation: &Operation,
    ) -> Result<Vec<LocatedElement>, Failure> {
        match &self.source {
            QuerySource::Browser(page) => {
                Ok(
                    execute(&BrowserSource(page.clone()), &self.query, action, operation)
                        .await?
                        .into_iter()
                        .map(LocatedElement::Browser)
                        .collect(),
                )
            }
            #[cfg(windows)]
            QuerySource::Uia {
                runtime,
                window,
                input,
            } => Ok(execute(
                &UiaSource {
                    runtime: runtime.clone(),
                    window: window.clone(),
                    input: input.clone(),
                },
                &self.query,
                action,
                operation,
            )
            .await?
            .into_iter()
            .map(LocatedElement::Uia)
            .collect()),
            #[cfg(windows)]
            QuerySource::Ocr {
                sampler,
                source,
                region,
                window,
                input,
            } => Ok(execute(
                &OcrSource {
                    sampler: sampler.clone(),
                    source: *source,
                    region: *region,
                    window: window.clone(),
                    input: input.clone(),
                },
                &self.query,
                action,
                operation,
            )
            .await?
            .into_iter()
            .map(|(sample, target)| LocatedElement::Ocr { sample, target })
            .collect()),
        }
    }
}
