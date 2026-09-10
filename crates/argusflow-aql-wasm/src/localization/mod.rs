//! 仅关键字本地化，用户内容和语法结构原样保留。
mod dictionary;
mod translation;
pub(crate) use dictionary::{chinese, english};
pub use translation::{Translation, localize, translate};
