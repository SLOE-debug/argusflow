use std::collections::{BTreeMap, BTreeSet, HashMap};

use argusflow_core::{
    ComponentInstance, FlowComponentDefinition, FlowComponentId, FlowComponentVersion,
    NodeEnvelope, ValueExpr, WorkflowDefinition, WorkflowEdge, WorkflowNode,
};
use serde::{Deserialize, Serialize};

use super::{
    component_registry::ComponentRegistry,
    component_rewrite::{rewrite_expression, rewrite_node},
};

/// P0 允许的最大组件嵌套层数。
pub const MAX_COMPONENT_DEPTH: usize = 8;

/// 一个展开节点在嵌套组件中的来源帧。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentSourceFrame {
    /// 当前层组件实例在展开图中的节点 ID。
    pub instance_node_id: String,
    /// 当前层组件稳定 ID。
    pub component_id: FlowComponentId,
    /// 当前层实例锁定的精确版本。
    pub component_version: FlowComponentVersion,
    /// 该节点在当前层组件定义中的直接内部节点 ID。
    pub inner_node_id: String,
}

/// 展开节点到完整组件嵌套路径的只读映射。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentSourceMap {
    /// 展开节点 ID 对应从外到内的组件来源路径。
    pub nodes: BTreeMap<String, Vec<ComponentSourceFrame>>,
}

impl ComponentSourceMap {
    /// 返回展开节点的只读来源路径。
    pub fn get(&self, expanded_node_id: &str) -> Option<&[ComponentSourceFrame]> {
        self.nodes.get(expanded_node_id).map(Vec::as_slice)
    }
}

/// 组件解析后可直接交给现有 Validator/Engine 的扁平工作流。
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedWorkflow {
    /// 不再包含 `argus.component` 实例的扁平 schema v9 工作流。
    pub definition: WorkflowDefinition,
    /// 执行事件映射回嵌套编辑器所需的来源表。
    pub source_map: ComponentSourceMap,
}

/// 递归解析并展开主流程中的全部组件实例。
pub fn expand_components(
    mut workflow: WorkflowDefinition,
    registry: &ComponentRegistry,
) -> Result<ExpandedWorkflow, ComponentExpansionError> {
    let mut source_map = ComponentSourceMap::default();
    expand_graph(
        &mut workflow.nodes,
        &mut workflow.edges,
        registry,
        &mut source_map,
    )?;
    Ok(ExpandedWorkflow {
        definition: workflow,
        source_map,
    })
}

/// 组件无法在进入现有 Runtime 前形成确定扁平图的错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentExpansionError {
    /// 相关父图组件实例节点 ID。
    pub node_id: Option<String>,
    /// 不包含业务值的安全错误说明。
    pub message: String,
}

impl ComponentExpansionError {
    /// 创建定位到组件实例的展开错误。
    fn at(node_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            node_id: Some(node_id.into()),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ComponentExpansionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ComponentExpansionError {}

/// 逐个展开当前图中的组件；新插入的嵌套实例会继续在同一循环中处理。
fn expand_graph(
    nodes: &mut Vec<WorkflowNode>,
    edges: &mut Vec<WorkflowEdge>,
    registry: &ComponentRegistry,
    source_map: &mut ComponentSourceMap,
) -> Result<(), ComponentExpansionError> {
    loop {
        let Some(index) = nodes.iter().position(is_component_node) else {
            return Ok(());
        };
        let instance_node = nodes[index].clone();
        let instance = decode_instance(&instance_node)?;
        let inherited_path = source_map
            .nodes
            .get(&instance_node.id)
            .cloned()
            .unwrap_or_default();
        if inherited_path.len() >= MAX_COMPONENT_DEPTH {
            return Err(ComponentExpansionError::at(
                &instance_node.id,
                format!("组件嵌套深度不能超过 {MAX_COMPONENT_DEPTH}"),
            ));
        }
        if inherited_path
            .iter()
            .any(|frame| frame.component_id == instance.component_id)
        {
            return Err(ComponentExpansionError::at(
                &instance_node.id,
                "组件不能直接或间接递归引用自身",
            ));
        }
        let definition = registry
            .resolve(instance.component_id, &instance.component_version)
            .ok_or_else(|| {
                ComponentExpansionError::at(
                    &instance_node.id,
                    format!(
                        "找不到锁定组件版本 '{}@{}'",
                        instance.component_id.as_uuid(),
                        instance.component_version.as_str(),
                    ),
                )
            })?;
        validate_component_definition(&instance_node.id, definition, &instance)?;
        expand_instance(
            nodes,
            edges,
            source_map,
            &instance_node,
            &instance,
            definition,
        )?;
    }
}

/// 把一个组件实例替换为内部节点、控制边和隐藏输出代理节点。
fn expand_instance(
    nodes: &mut Vec<WorkflowNode>,
    edges: &mut Vec<WorkflowEdge>,
    source_map: &mut ComponentSourceMap,
    instance_node: &WorkflowNode,
    instance: &ComponentInstance,
    definition: &FlowComponentDefinition,
) -> Result<(), ComponentExpansionError> {
    let inherited_path = source_map
        .nodes
        .remove(&instance_node.id)
        .unwrap_or_default();
    let id_map = definition
        .nodes
        .iter()
        .filter(|node| node.id != definition.entry_node_id && node.id != definition.exit_node_id)
        .map(|node| (node.id.clone(), namespace(&instance_node.id, &node.id)))
        .collect::<HashMap<_, _>>();

    let mut expanded_nodes = definition
        .nodes
        .iter()
        .filter(|node| node.id != definition.entry_node_id && node.id != definition.exit_node_id)
        .map(|node| {
            let expanded_id = id_map.get(&node.id).cloned().ok_or_else(|| {
                ComponentExpansionError::at(&instance_node.id, "组件节点 ID namespace 构造失败")
            })?;
            let mut expanded = rewrite_node(node, &expanded_id, &instance.inputs, &id_map)?;
            expanded.position.x += instance_node.position.x;
            expanded.position.y += instance_node.position.y;
            let mut path = inherited_path.clone();
            path.push(ComponentSourceFrame {
                instance_node_id: instance_node.id.clone(),
                component_id: definition.id,
                component_version: definition.version.clone(),
                inner_node_id: node.id.clone(),
            });
            source_map.nodes.insert(expanded_id, path);
            Ok(expanded)
        })
        .collect::<Result<Vec<_>, ComponentExpansionError>>()?;

    let outputs = definition
        .outputs
        .iter()
        .map(|output| {
            rewrite_expression(&output.value, &instance.inputs, &id_map)
                .map(|value| (output.name.clone(), value))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    expanded_nodes.push(WorkflowNode {
        id: instance_node.id.clone(),
        position: instance_node.position,
        definition: NodeEnvelope::from_payload(
            "argus.component.output",
            1,
            &ComponentOutputPayload { outputs },
        )
        .map_err(|error| ComponentExpansionError::at(&instance_node.id, error.to_string()))?,
        output_bindings: instance_node.output_bindings.clone(),
    });
    let mut output_path = inherited_path;
    output_path.push(ComponentSourceFrame {
        instance_node_id: instance_node.id.clone(),
        component_id: definition.id,
        component_version: definition.version.clone(),
        inner_node_id: definition.exit_node_id.clone(),
    });
    source_map
        .nodes
        .insert(instance_node.id.clone(), output_path);

    let entry_targets = definition
        .edges
        .iter()
        .filter(|edge| edge.source == definition.entry_node_id)
        .map(|edge| boundary_target(edge, definition, &id_map, &instance_node.id))
        .collect::<Result<Vec<_>, _>>()?;
    let [entry_target] = entry_targets.as_slice() else {
        return Err(ComponentExpansionError::at(
            &instance_node.id,
            "组件入口必须且只能有一条出边",
        ));
    };

    let mut expanded_edges = Vec::new();
    for edge in edges.iter() {
        if edge.target == instance_node.id {
            let mut rewritten = edge.clone();
            rewritten.target = entry_target.clone();
            expanded_edges.push(rewritten);
        } else {
            expanded_edges.push(edge.clone());
        }
    }
    for edge in &definition.edges {
        if edge.source == definition.entry_node_id {
            continue;
        }
        if edge.target == definition.entry_node_id || edge.source == definition.exit_node_id {
            return Err(ComponentExpansionError::at(
                &instance_node.id,
                "组件边不能进入入口或离开出口",
            ));
        }
        let source = id_map.get(&edge.source).cloned().ok_or_else(|| {
            ComponentExpansionError::at(
                &instance_node.id,
                format!("组件边引用未知起点 '{}'", edge.source),
            )
        })?;
        let target = boundary_target(edge, definition, &id_map, &instance_node.id)?;
        expanded_edges.push(WorkflowEdge {
            id: namespace(&instance_node.id, &edge.id),
            source,
            target,
            branch: edge.branch.clone(),
        });
    }

    let instance_index = nodes
        .iter()
        .position(|node| node.id == instance_node.id)
        .ok_or_else(|| ComponentExpansionError::at(&instance_node.id, "组件实例在展开期间消失"))?;
    nodes.remove(instance_index);
    nodes.extend(expanded_nodes);
    *edges = expanded_edges;
    Ok(())
}

/// 验证不依赖现有 NodeRegistry 的组件发布边界约束。
fn validate_component_definition(
    instance_node_id: &str,
    definition: &FlowComponentDefinition,
    instance: &ComponentInstance,
) -> Result<(), ComponentExpansionError> {
    if definition.schema_version != 1 {
        return Err(ComponentExpansionError::at(
            instance_node_id,
            "组合步骤的格式版本必须为 1",
        ));
    }
    if definition.name.trim().is_empty() {
        return Err(ComponentExpansionError::at(
            instance_node_id,
            "组件名称不能为空",
        ));
    }
    let node_ids = definition
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    if node_ids.len() != definition.nodes.len() {
        return Err(ComponentExpansionError::at(
            instance_node_id,
            "组件内部节点 ID 必须唯一",
        ));
    }
    let entry = definition
        .nodes
        .iter()
        .find(|node| node.id == definition.entry_node_id);
    let exit = definition
        .nodes
        .iter()
        .find(|node| node.id == definition.exit_node_id);
    if entry.is_none_or(|node| node.definition.type_id.as_str() != "argus.start")
        || exit.is_none_or(|node| node.definition.type_id.as_str() != "argus.end")
    {
        return Err(ComponentExpansionError::at(
            instance_node_id,
            "组件入口和出口必须分别引用 argus.start 与 argus.end 边界节点",
        ));
    }
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

/// 解码 `argus.component` v1 payload。
fn decode_instance(node: &WorkflowNode) -> Result<ComponentInstance, ComponentExpansionError> {
    if node.definition.version != 1 {
        return Err(ComponentExpansionError::at(
            &node.id,
            "组合步骤的设置版本必须为 1",
        ));
    }
    serde_json::from_value(node.definition.payload.clone()).map_err(|error| {
        ComponentExpansionError::at(&node.id, format!("组合步骤的设置格式不正确：{error}"))
    })
}

/// 把内部边目标改写为 namespace ID；出口边统一进入组件输出代理。
fn boundary_target(
    edge: &WorkflowEdge,
    definition: &FlowComponentDefinition,
    id_map: &HashMap<String, String>,
    instance_node_id: &str,
) -> Result<String, ComponentExpansionError> {
    if edge.target == definition.exit_node_id {
        return Ok(instance_node_id.to_owned());
    }
    id_map.get(&edge.target).cloned().ok_or_else(|| {
        ComponentExpansionError::at(
            instance_node_id,
            format!("组件边引用未知目标 '{}'", edge.target),
        )
    })
}

/// 使用稳定分隔符生成嵌套节点和边 ID。
fn namespace(instance_node_id: &str, inner_id: &str) -> String {
    format!("{instance_node_id}::{inner_id}")
}

/// 判断节点是否仍是等待展开的组件实例。
fn is_component_node(node: &WorkflowNode) -> bool {
    node.definition.type_id.as_str() == "argus.component"
}

/// 生成隐藏输出代理节点时使用的强类型 payload。
#[derive(Debug, Serialize)]
struct ComponentOutputPayload {
    /// 组件公开输出名到内部表达式的冻结映射。
    outputs: BTreeMap<String, ValueExpr>,
}
