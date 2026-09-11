//! 通过公开 API 验证编译与运行语义。
#[path = "../unit/bundle.rs"]
mod bundle;
#[path = "../unit/control.rs"]
mod control;
#[path = "../unit/expressions.rs"]
mod expressions;
#[path = "../unit/failure.rs"]
mod failure;
#[path = "../unit/limits.rs"]
mod limits;
#[path = "../unit/scopes.rs"]
mod scopes;
#[path = "../support/builders.rs"]
mod support;
#[path = "../unit/tasks.rs"]
mod task_tests;
#[path = "../support/tasks.rs"]
mod tasks;
#[path = "../unit/unwind.rs"]
mod unwind;
