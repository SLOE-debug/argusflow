mod application;
mod browser;
mod browser_operation;
mod command;
mod component_output;
mod control;
mod data_format;
mod observe;
mod ui;
mod utility;
mod variable;

use std::{marker::PhantomData, sync::Arc};

use argusflow_core::{
    ApplicationSessionProvider, BrowserSessionProvider, NodeEnvelope, NodeTypeId,
};
use serde::de::DeserializeOwned;

use crate::{ActionDispatcher, NodeCompileError, NodeCompiler, NodeTypeRegistry, PreparedNode};

/// 为运行时装配全部内置节点编译器；每项编译器只拥有自己的依赖和 payload 类型。
pub(crate) fn registry(
    dispatcher: Arc<dyn ActionDispatcher>,
    observations: Arc<dyn crate::ObservationDispatcher>,
    applications: Arc<dyn ApplicationSessionProvider>,
    browsers: Arc<dyn BrowserSessionProvider>,
) -> NodeTypeRegistry {
    let browser_operations = Arc::clone(&browsers);
    NodeTypeRegistry::from_builtin_compilers([
        typed_compiler::<control::StartPayload>("argus.start", control::prepare_start),
        typed_compiler::<utility::LogPayload>("argus.log", utility::prepare_log),
        typed_compiler::<utility::DebugPayload>("argus.debug", utility::prepare_debug),
        typed_compiler::<utility::DelayPayload>("argus.delay", utility::prepare_delay),
        typed_compiler::<control::ConditionPayload>("argus.condition", control::prepare_condition),
        typed_compiler_at_version::<control::LoopPayload>("argus.loop", 2, control::prepare_loop),
        typed_compiler::<control::LoopEntryPayload>(
            "argus.loop.entry",
            control::prepare_loop_entry,
        ),
        typed_compiler::<control::LoopContinuePayload>(
            "argus.loop.continue",
            control::prepare_loop_continue,
        ),
        typed_compiler::<control::LoopCompletePayload>(
            "argus.loop.complete",
            control::prepare_loop_complete,
        ),
        typed_compiler::<variable::SetVariablesPayload>("argus.variable.set", variable::prepare),
        typed_compiler::<component_output::ComponentOutputPayload>(
            "argus.component.output",
            component_output::prepare,
        ),
        typed_compiler::<data_format::DataFormatPayload>("argus.data.format", data_format::prepare),
        application::compiler(applications),
        browser::compiler(browsers),
        browser_operation::compiler(browser_operations),
        observe::compiler(observations),
        ui::compiler(dispatcher),
        command::compiler(),
        typed_compiler::<control::FailPayload>("argus.fail", control::prepare_fail),
        typed_compiler::<control::EndPayload>("argus.end", control::prepare_end),
    ])
}

/// 建立只负责版本检查、serde 解码与强类型构造的内置编译器。
pub(super) fn typed_compiler<Payload>(
    type_id: &'static str,
    prepare: fn(Payload) -> Arc<dyn PreparedNode>,
) -> Arc<dyn NodeCompiler>
where
    Payload: DeserializeOwned + Send + Sync + 'static,
{
    typed_compiler_at_version(type_id, 1, prepare)
}

/// 建立拥有显式 payload 版本的强类型编译器。
pub(super) fn typed_compiler_at_version<Payload>(
    type_id: &'static str,
    version: u16,
    prepare: fn(Payload) -> Arc<dyn PreparedNode>,
) -> Arc<dyn NodeCompiler>
where
    Payload: DeserializeOwned + Send + Sync + 'static,
{
    Arc::new(TypedNodeCompiler {
        type_id: NodeTypeId::new(type_id),
        version,
        prepare,
        payload: PhantomData,
    })
}

/// 内置节点共享的单版本强类型 JSON 编译器。
struct TypedNodeCompiler<Payload> {
    /// 注册表查找使用的稳定 ID。
    type_id: NodeTypeId,
    /// 唯一接受的当前 payload 版本。
    version: u16,
    /// 将解码后的 payload 冻结为节点执行对象。
    prepare: fn(Payload) -> Arc<dyn PreparedNode>,
    /// 只表达编译器拥有的 payload 类型，不保存实例。
    payload: PhantomData<fn() -> Payload>,
}

impl<Payload> NodeCompiler for TypedNodeCompiler<Payload>
where
    Payload: DeserializeOwned + Send + Sync + 'static,
{
    fn type_id(&self) -> &NodeTypeId {
        &self.type_id
    }

    fn compile(
        &self,
        definition: &NodeEnvelope,
    ) -> Result<Arc<dyn PreparedNode>, NodeCompileError> {
        if definition.version != self.version {
            return Err(NodeCompileError::new(format!(
                "unsupported payload version {}; expected {}",
                definition.version, self.version,
            )));
        }
        let payload =
            serde_json::from_value::<Payload>(definition.payload.clone()).map_err(|error| {
                NodeCompileError::new(format!("payload does not match registered schema: {error}"))
            })?;
        Ok((self.prepare)(payload))
    }
}
