//! 不依赖平台的表达式输出与比较类型检查。
mod expression;
mod operator;
pub(crate) use expression::validate;
pub(crate) use operator::accepts;
