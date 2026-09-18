//! 仅关键字本地化，用户内容和语法结构原样保留。
mod dictionary;
mod target;
mod translation;
pub(crate) use dictionary::{chinese, english};
pub use target::{compile_target, is_target_symbol, translate_target};
pub use translation::{Translation, localize, translate};
