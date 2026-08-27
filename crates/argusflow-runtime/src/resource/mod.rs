//! 外部资源表、访问仲裁和生命周期清理。

pub(crate) mod resource_cleanup;
pub(crate) mod resource_table;

pub(crate) use resource_cleanup::{ApplicationResourceCleanup, BrowserResourceCleanup};
pub use resource_table::{ResourceCleanup, ResourceEntry, ResourceTable};
