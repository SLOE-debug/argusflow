//! 开放任务在编译后不再接收动态配置或整个运行上下文。
use crate::{Resource, RunError};
use argusflow_core::Operation;
use argusflow_workflow::{Fields, ResourceFields, Task, Values};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

/// 对象安全的异步能力调用。
pub type TaskFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, RunError>> + Send + 'a>>;

/// 编译后的任务端口及重试声明。
#[derive(Debug, Clone, Default)]
pub struct TaskSignature {
    /// 数据输入的完整类型。
    pub inputs: Fields,
    /// 原生数据输出的完整类型。
    pub outputs: Fields,
    /// 显式借用的资源端口。
    pub resources: ResourceFields,
    /// 成功后创建的资源端口。
    pub resource_outputs: ResourceFields,
    /// 仅声明没有业务副作用、可以安全重复的任务。
    pub safe_to_retry: bool,
}

/// 对任务开放的不可变数据输入及资源借用。
pub struct TaskContext<'a> {
    /// 已求值且通过类型验证的输入。
    pub inputs: &'a Values,
    /// 仅包含签名声明端口的资源，不暴露资源表。
    pub resources: &'a BTreeMap<String, Arc<dyn Resource>>,
    /// 当前尝试的独立操作票据。
    pub operation: &'a Operation,
}

/// 原生任务结果；引擎校验后再发布，资源与数据分离。
#[derive(Default)]
pub struct TaskOutput {
    /// 所有声明的数据输出。
    pub values: Values,
    /// 转移给当前作用域的所有声明资源。
    pub resources: BTreeMap<String, Arc<dyn Resource>>,
}

/// 一个准备好的扩展任务；实现必须协作取消并清理未交付的资源。
pub trait PreparedTask: Send + Sync {
    /// 冻结端口与行为约束，执行期间不得改变。
    fn signature(&self) -> TaskSignature;
    /// 不得保存 context 借用，不得修改调用方变量。
    fn execute<'a>(&'a self, context: TaskContext<'a>) -> TaskFuture<'a, TaskOutput>;
}

/// 每个真实业务模块拥有其类型 ID 与静态配置解码。
pub trait NodeCompiler: Send + Sync {
    /// 模块拥有的唯一类型名称。
    fn type_id(&self) -> &str;
    /// 只在准备阶段调用，应拒绝未知版本及配置字段。
    fn compile(
        &self,
        version: u16,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn PreparedTask>, String>;
}

/// 宿主装配的开放任务注册表；重复注册明确失败。
#[derive(Default)]
pub struct NodeRegistry {
    compilers: BTreeMap<String, Arc<dyn NodeCompiler>>,
}
impl NodeRegistry {
    /// 空注册表，所有内置控制结构无需注册。
    pub fn new() -> Self {
        Self::default()
    }
    /// 添加一个编译器；拒绝重复、空白或过长名称。
    pub fn register(&mut self, compiler: Arc<dyn NodeCompiler>) -> Result<(), String> {
        let id = compiler.type_id();
        if id.trim().is_empty() || id.len() > 256 || self.compilers.contains_key(id) {
            return Err("任务类型为空、过长或重复".into());
        }
        self.compilers.insert(id.to_owned(), compiler);
        Ok(())
    }
    pub(crate) fn compile(
        &self,
        task: &Task,
    ) -> Result<(Arc<dyn PreparedTask>, TaskSignature), String> {
        let compiler = self
            .compilers
            .get(&task.type_id)
            .ok_or_else(|| format!("未注册任务类型：{}", task.type_id))?;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let prepared = compiler.compile(task.version, &task.config)?;
            let signature = prepared.signature();
            Ok((prepared, signature))
        }))
        .map_err(|_| "扩展编译器 panic，准备已终止".to_owned())?
    }
}
