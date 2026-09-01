//! 把可复用组件展开到多作用域图，并保留稳定的执行来源映射。

use std::collections::{BTreeMap, HashMap};

use argusflow_core::{
    ComponentInstance, FlowComponentDefinition, FlowComponentId, FlowComponentVersion, FlowScope,
    FlowScopeBoundary, FlowScopeParent, NodeEnvelope, WorkflowDefinition, WorkflowEdge,
    WorkflowNode,
};
use serde::{Deserialize, Serialize};

use super::{
    component_contract::{ComponentOutputPayload, decode_instance, validate_component_definition},
    component_registry::ComponentRegistry,
    component_rewrite::{rewrite_expression, rewrite_node},
};

/// 组件允许的最大嵌套层数；While 嵌套不计入该限制。
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

/// 组件解析后可直接交给 Validator/Engine 的多作用域工作流。
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedWorkflow {
    /// 不再包含 `argus.component` 实例的 schema v10 工作流。
    pub definition: WorkflowDefinition,
    /// 执行事件映射回嵌套编辑器所需的来源表。
    pub source_map: ComponentSourceMap,
}

/// 组件展开失败时的可定位错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentExpansionError {
    /// 相关父图组件实例节点 ID。
    pub node_id: Option<String>,
    /// 不包含业务值的安全错误说明。
    pub message: String,
}

impl ComponentExpansionError {
    /// 创建定位到组件实例的展开错误。
    pub(super) fn at(node_id: impl Into<String>, message: impl Into<String>) -> Self {
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

/// 迭代展开全部作用域中的组件实例，避免组件或 While 深度进入调用栈。
pub fn expand_components(
    mut workflow: WorkflowDefinition,
    registry: &ComponentRegistry,
) -> Result<ExpandedWorkflow, ComponentExpansionError> {
    let mut source_map = ComponentSourceMap::default();
    loop {
        let next_instance =
            workflow
                .graph
                .scopes
                .iter()
                .enumerate()
                .find_map(|(scope_index, scope)| {
                    scope
                        .nodes
                        .iter()
                        .enumerate()
                        .find_map(|(node_index, node)| {
                            is_component_node(node).then_some((scope_index, node_index))
                        })
                });
        let Some((scope_index, node_index)) = next_instance else {
            break;
        };
        expand_instance(
            &mut workflow,
            scope_index,
            node_index,
            registry,
            &mut source_map,
        )?;
    }
    Ok(ExpandedWorkflow {
        definition: workflow,
        source_map,
    })
}

/// 把单个组件实例的根图拼入当前作用域，并导入其全部 While 子作用域。
fn expand_instance(
    workflow: &mut WorkflowDefinition,
    scope_index: usize,
    node_index: usize,
    registry: &ComponentRegistry,
    source_map: &mut ComponentSourceMap,
) -> Result<(), ComponentExpansionError> {
    let instance_node = workflow.graph.scopes[scope_index].nodes[node_index].clone();
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
    let (root_scope, entry_node_id, exit_node_id) =
        validate_component_definition(&instance_node.id, definition, &instance)?;
    let scope_id_map = definition
        .graph
        .scopes
        .iter()
        .map(|scope| (scope.id.clone(), namespace(&instance_node.id, &scope.id)))
        .collect::<HashMap<_, _>>();
    let node_id_map = definition
        .graph
        .scopes
        .iter()
        .flat_map(|scope| scope.nodes.iter())
        .filter(|node| node.id != entry_node_id && node.id != exit_node_id)
        .map(|node| (node.id.clone(), namespace(&instance_node.id, &node.id)))
        .collect::<HashMap<_, _>>();

    let mut root_nodes = rewrite_nodes(
        root_scope,
        &instance_node,
        &instance,
        definition,
        &inherited_path,
        &node_id_map,
        &scope_id_map,
        source_map,
        true,
    )?;
    let outputs = definition
        .outputs
        .iter()
        .map(|output| {
            rewrite_expression(&output.value, &instance.inputs, &node_id_map)
                .map(|value| (output.name.clone(), value))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    root_nodes.push(WorkflowNode {
        id: instance_node.id.clone(),
        position: instance_node.position,
        size: instance_node.size,
        definition: NodeEnvelope::from_payload(
            "argus.component.output",
            1,
            &ComponentOutputPayload { outputs },
        )
        .map_err(|error| ComponentExpansionError::at(&instance_node.id, error.to_string()))?,
        output_bindings: instance_node.output_bindings.clone(),
    });
    record_source(
        source_map,
        &instance_node.id,
        &inherited_path,
        &instance_node,
        definition,
        exit_node_id,
    );

    let entry_targets = root_scope
        .edges
        .iter()
        .filter(|edge| edge.source == entry_node_id)
        .map(|edge| boundary_target(edge, exit_node_id, &node_id_map, &instance_node.id))
        .collect::<Result<Vec<_>, _>>()?;
    let [entry_target] = entry_targets.as_slice() else {
        return Err(ComponentExpansionError::at(
            &instance_node.id,
            "组件入口必须且只能有一条出边",
        ));
    };
    let current_scope_id = workflow.graph.scopes[scope_index].id.clone();
    let existing_edges = workflow.graph.scopes[scope_index].edges.clone();
    let mut expanded_edges = existing_edges
        .into_iter()
        .map(|mut edge| {
            if edge.target == instance_node.id {
                edge.target = entry_target.clone();
            }
            edge
        })
        .collect::<Vec<_>>();
    expanded_edges.extend(rewrite_root_edges(
        root_scope,
        entry_node_id,
        exit_node_id,
        &instance_node.id,
        &node_id_map,
    )?);

    let mut imported_scopes = Vec::new();
    for scope in definition
        .graph
        .scopes
        .iter()
        .filter(|scope| scope.id != root_scope.id)
    {
        imported_scopes.push(rewrite_child_scope(
            scope,
            &current_scope_id,
            &instance_node,
            &instance,
            definition,
            &inherited_path,
            &node_id_map,
            &scope_id_map,
            source_map,
        )?);
    }
    let current_scope = &mut workflow.graph.scopes[scope_index];
    current_scope.nodes.remove(node_index);
    current_scope.nodes.extend(root_nodes);
    current_scope.edges = expanded_edges;
    workflow.graph.scopes.extend(imported_scopes);
    Ok(())
}

/// 重写一个组件作用域中的节点，并记录每个执行节点的组件来源。
#[allow(clippy::too_many_arguments)]
fn rewrite_nodes(
    scope: &FlowScope,
    instance_node: &WorkflowNode,
    instance: &ComponentInstance,
    definition: &FlowComponentDefinition,
    inherited_path: &[ComponentSourceFrame],
    node_id_map: &HashMap<String, String>,
    scope_id_map: &HashMap<String, String>,
    source_map: &mut ComponentSourceMap,
    skip_component_boundary: bool,
) -> Result<Vec<WorkflowNode>, ComponentExpansionError> {
    let boundary_ids = match &scope.boundary {
        FlowScopeBoundary::Component {
            entry_node_id,
            exit_node_id,
        } if skip_component_boundary => Some((entry_node_id.as_str(), exit_node_id.as_str())),
        _ => None,
    };
    scope
        .nodes
        .iter()
        .filter(|node| boundary_ids.is_none_or(|(entry, exit)| node.id != entry && node.id != exit))
        .map(|node| {
            let expanded_id = mapped(node_id_map, &node.id, &instance_node.id)?;
            let mut expanded = rewrite_node(
                node,
                expanded_id,
                &instance.inputs,
                node_id_map,
                scope_id_map,
            )?;
            if skip_component_boundary {
                expanded.position.x += instance_node.position.x;
                expanded.position.y += instance_node.position.y;
            }
            record_source(
                source_map,
                expanded_id,
                inherited_path,
                instance_node,
                definition,
                &node.id,
            );
            Ok(expanded)
        })
        .collect()
}

/// 导入一个组件 While 子作用域并重写其所有结构 ID。
#[allow(clippy::too_many_arguments)]
fn rewrite_child_scope(
    scope: &FlowScope,
    containing_scope_id: &str,
    instance_node: &WorkflowNode,
    instance: &ComponentInstance,
    definition: &FlowComponentDefinition,
    inherited_path: &[ComponentSourceFrame],
    node_id_map: &HashMap<String, String>,
    scope_id_map: &HashMap<String, String>,
    source_map: &mut ComponentSourceMap,
) -> Result<FlowScope, ComponentExpansionError> {
    let parent = scope
        .parent
        .as_ref()
        .ok_or_else(|| ComponentExpansionError::at(&instance_node.id, "组件子作用域缺少父容器"))?;
    let parent_scope_id = if parent.scope_id == definition.graph.root_scope_id {
        containing_scope_id.to_owned()
    } else {
        mapped(scope_id_map, &parent.scope_id, &instance_node.id)?.to_owned()
    };
    let nodes = rewrite_nodes(
        scope,
        instance_node,
        instance,
        definition,
        inherited_path,
        node_id_map,
        scope_id_map,
        source_map,
        false,
    )?;
    let edges = scope
        .edges
        .iter()
        .map(|edge| {
            Ok(WorkflowEdge {
                id: namespace(&instance_node.id, &edge.id),
                source: mapped(node_id_map, &edge.source, &instance_node.id)?.to_owned(),
                target: mapped(node_id_map, &edge.target, &instance_node.id)?.to_owned(),
                branch: edge.branch.clone(),
            })
        })
        .collect::<Result<Vec<_>, ComponentExpansionError>>()?;
    Ok(FlowScope {
        id: mapped(scope_id_map, &scope.id, &instance_node.id)?.to_owned(),
        parent: Some(FlowScopeParent {
            scope_id: parent_scope_id,
            node_id: mapped(node_id_map, &parent.node_id, &instance_node.id)?.to_owned(),
        }),
        boundary: rewrite_boundary(&scope.boundary, node_id_map, &instance_node.id)?,
        nodes,
        edges,
    })
}

/// 重写组件根图中除入口边以外的控制边。
fn rewrite_root_edges(
    scope: &FlowScope,
    entry_node_id: &str,
    exit_node_id: &str,
    instance_node_id: &str,
    node_id_map: &HashMap<String, String>,
) -> Result<Vec<WorkflowEdge>, ComponentExpansionError> {
    scope
        .edges
        .iter()
        .filter(|edge| edge.source != entry_node_id)
        .map(|edge| {
            if edge.target == entry_node_id || edge.source == exit_node_id {
                return Err(ComponentExpansionError::at(
                    instance_node_id,
                    "组件边不能进入入口或离开出口",
                ));
            }
            Ok(WorkflowEdge {
                id: namespace(instance_node_id, &edge.id),
                source: mapped(node_id_map, &edge.source, instance_node_id)?.to_owned(),
                target: boundary_target(edge, exit_node_id, node_id_map, instance_node_id)?,
                branch: edge.branch.clone(),
            })
        })
        .collect()
}

/// 把内部边目标改写为 namespace ID；出口边统一进入组件输出代理。
fn boundary_target(
    edge: &WorkflowEdge,
    exit_node_id: &str,
    node_id_map: &HashMap<String, String>,
    instance_node_id: &str,
) -> Result<String, ComponentExpansionError> {
    if edge.target == exit_node_id {
        return Ok(instance_node_id.to_owned());
    }
    mapped(node_id_map, &edge.target, instance_node_id).map(str::to_owned)
}

/// 重写子作用域的强类型边界节点 ID。
fn rewrite_boundary(
    boundary: &FlowScopeBoundary,
    node_id_map: &HashMap<String, String>,
    instance_node_id: &str,
) -> Result<FlowScopeBoundary, ComponentExpansionError> {
    let map = |id: &str| mapped(node_id_map, id, instance_node_id).map(str::to_owned);
    match boundary {
        FlowScopeBoundary::Workflow { entry_node_id } => Ok(FlowScopeBoundary::Workflow {
            entry_node_id: map(entry_node_id)?,
        }),
        FlowScopeBoundary::Component {
            entry_node_id,
            exit_node_id,
        } => Ok(FlowScopeBoundary::Component {
            entry_node_id: map(entry_node_id)?,
            exit_node_id: map(exit_node_id)?,
        }),
        FlowScopeBoundary::Loop {
            entry_node_id,
            continue_node_id,
            complete_node_id,
        } => Ok(FlowScopeBoundary::Loop {
            entry_node_id: map(entry_node_id)?,
            continue_node_id: map(continue_node_id)?,
            complete_node_id: map(complete_node_id)?,
        }),
    }
}

/// 记录展开节点从外到内的完整组件路径。
fn record_source(
    source_map: &mut ComponentSourceMap,
    expanded_id: &str,
    inherited_path: &[ComponentSourceFrame],
    instance_node: &WorkflowNode,
    definition: &FlowComponentDefinition,
    inner_node_id: &str,
) {
    let mut path = inherited_path.to_vec();
    path.push(ComponentSourceFrame {
        instance_node_id: instance_node.id.clone(),
        component_id: definition.id,
        component_version: definition.version.clone(),
        inner_node_id: inner_node_id.to_owned(),
    });
    source_map.nodes.insert(expanded_id.to_owned(), path);
}

/// 返回一个必需的 ID 映射，失败时定位到组件实例。
fn mapped<'a>(
    map: &'a HashMap<String, String>,
    id: &str,
    instance_node_id: &str,
) -> Result<&'a str, ComponentExpansionError> {
    map.get(id).map(String::as_str).ok_or_else(|| {
        ComponentExpansionError::at(instance_node_id, format!("组件结构引用未知 ID '{id}'"))
    })
}

/// 使用稳定分隔符生成嵌套作用域、节点和边 ID。
fn namespace(instance_node_id: &str, inner_id: &str) -> String {
    format!("{instance_node_id}::{inner_id}")
}

/// 判断节点是否仍是等待展开的组件实例。
fn is_component_node(node: &WorkflowNode) -> bool {
    node.definition.type_id.as_str() == "argus.component"
}
