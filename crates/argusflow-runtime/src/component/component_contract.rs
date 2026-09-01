//! 可复用组件 v2 持久化契约和实例绑定校验。

use std::collections::BTreeSet;

use argusflow_core::{
    ComponentInstance, FlowComponentDefinition, FlowScope, FlowScopeBoundary, WorkflowNode,
};

use super::component_expander::ComponentExpansionError;

/// 生成隐藏输出代理节点时使用的强类型 payload。
#[derive(Debug, serde::Serialize)]
pub(super) struct ComponentOutputPayload {
    /// 组件公开输出名到内部表达式的冻结映射。
    pub(super) outputs: std::collections::BTreeMap<String, argusflow_core::ValueExpr>,
}

/// 验证组件 v2 的根边界与实例输入契约。
pub(super) fn validate_component_definition<'a>(
    instance_node_id: &str,
    definition: &'a FlowComponentDefinition,
    instance: &ComponentInstance,
) -> Result<(&'a FlowScope, &'a str, &'a str), ComponentExpansionError> {
    if definition.schema_version != 2 {
        return Err(ComponentExpansionError::at(
            instance_node_id,
            "组合步骤的格式版本必须为 2",
        ));
    }
    if definition.name.trim().is_empty() {
        return Err(ComponentExpansionError::at(
            instance_node_id,
            "组件名称不能为空",
        ));
    }
    let root = definition
        .graph
        .scopes
        .iter()
        .find(|scope| scope.id == definition.graph.root_scope_id)
        .ok_or_else(|| ComponentExpansionError::at(instance_node_id, "组件根作用域不存在"))?;
    let FlowScopeBoundary::Component {
        entry_node_id,
        exit_node_id,
    } = &root.boundary
    else {
        return Err(ComponentExpansionError::at(
            instance_node_id,
            "组件根作用域必须使用 Component 边界",
        ));
    };
    let node_ids = definition
        .graph
        .scopes
        .iter()
        .flat_map(|scope| scope.nodes.iter())
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    let node_count = definition
        .graph
        .scopes
        .iter()
        .map(|scope| scope.nodes.len())
        .sum::<usize>();
    if node_ids.len() != node_count {
        return Err(ComponentExpansionError::at(
            instance_node_id,
            "组件内部节点 ID 必须全局唯一",
        ));
    }
    let entry = root.nodes.iter().find(|node| node.id == *entry_node_id);
    let exit = root.nodes.iter().find(|node| node.id == *exit_node_id);
    if entry.is_none_or(|node| node.definition.type_id.as_str() != "argus.start")
        || exit.is_none_or(|node| node.definition.type_id.as_str() != "argus.end")
    {
        return Err(ComponentExpansionError::at(
            instance_node_id,
            "组件入口和出口必须分别引用 argus.start 与 argus.end 边界节点",
        ));
    }
    validate_component_ports(instance_node_id, definition, instance)?;
    Ok((root, entry_node_id, exit_node_id))
}

/// 校验组件输入绑定与公开输出名称。
fn validate_component_ports(
    instance_node_id: &str,
    definition: &FlowComponentDefinition,
    instance: &ComponentInstance,
) -> Result<(), ComponentExpansionError> {
    let input_names = definition
        .inputs
        .iter()
        .map(|input| input.key.as_str())
        .collect::<BTreeSet<_>>();
    if input_names.len() != definition.inputs.len()
        || definition
            .inputs
            .iter()
            .any(|input| input.key.trim().is_empty())
        || instance
            .inputs
            .keys()
            .any(|name| !input_names.contains(name.as_str()))
        || definition
            .inputs
            .iter()
            .any(|input| !instance.inputs.contains_key(&input.key))
    {
        return Err(ComponentExpansionError::at(
            instance_node_id,
            "组件实例必须且只能绑定定义声明的全部输入",
        ));
    }
    let output_names = definition
        .outputs
        .iter()
        .map(|output| output.name.as_str())
        .collect::<BTreeSet<_>>();
    if output_names.len() != definition.outputs.len()
        || definition
            .outputs
            .iter()
            .any(|output| output.name.trim().is_empty())
    {
        return Err(ComponentExpansionError::at(
            instance_node_id,
            "组件输出名称必须非空且唯一",
        ));
    }
    Ok(())
}

/// 解码 `argus.component` v2 payload。
pub(super) fn decode_instance(
    node: &WorkflowNode,
) -> Result<ComponentInstance, ComponentExpansionError> {
    if node.definition.version != 2 {
        return Err(ComponentExpansionError::at(
            &node.id,
            "组合步骤的设置版本必须为 2",
        ));
    }
    serde_json::from_value(node.definition.payload.clone()).map_err(|error| {
        ComponentExpansionError::at(&node.id, format!("组合步骤的设置格式不正确：{error}"))
    })
}
