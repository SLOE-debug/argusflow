//! 浏览器动作沿用其连接级输入排他和文档身份。
use super::{SourceBackend, contract::SourceFuture};
use argusflow_aql::BoundQuery;
use argusflow_browser::{BrowserMatch, Page};
use argusflow_core::Operation;

pub(crate) struct BrowserSource(pub Page);
impl SourceBackend for BrowserSource {
    type Target = BrowserMatch;
    fn find<'a>(
        &'a self,
        query: &'a BoundQuery,
        operation: &'a Operation,
    ) -> SourceFuture<'a, Vec<BrowserMatch>> {
        Box::pin(async move { self.0.query_aql(query, operation).await.map_err(Into::into) })
    }
    fn click<'a>(
        &'a self,
        target: &'a BrowserMatch,
        operation: &'a Operation,
    ) -> SourceFuture<'a, ()> {
        Box::pin(async move { target.click_aql(operation).await.map_err(Into::into) })
    }
    fn type_text<'a>(
        &'a self,
        target: &'a BrowserMatch,
        text: &'a str,
        operation: &'a Operation,
    ) -> SourceFuture<'a, ()> {
        Box::pin(async move {
            target
                .type_text_aql(text, operation)
                .await
                .map_err(Into::into)
        })
    }
}
