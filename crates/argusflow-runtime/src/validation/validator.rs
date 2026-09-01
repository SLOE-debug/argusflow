use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use argusflow_core::{FlowComponentDefinition, WorkflowDefinition};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    validation_graph::{build_graph, validate_graph_shape, validate_node_degrees},
    validation_references::validate_data_references,
    validation_scopes::validate_scopes,
    validation_workflow::{
        annotate_issue_locations, optional_scope_boundaries, scope_terminals,
        validate_global_edge_ids, validate_terminal_counts, validate_workflow_metadata,
    },
};
use crate::{
    application::UnavailableApplicationSessionProvider,
    browser::UnavailableBrowserSessionProvider,
    builtin_nodes,
    component::{ComponentRegistry, ComponentSourceMap, expand_components},
    execution::UnavailableActionDispatcher,
    node_registry::{
        NodeFlow, NodeRegistryError, NodeTypeRegistry, NodeValidationContext, PreparedNode,
    },
    value_runtime::{RuntimeValuePlan, RuntimeValuePlanBuilder},
};

/// 工作流结构校验的汇总结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// 没有任何问题时为 `true`。
    pub valid: bool,
    /// 按校验顺序收集的全部问题。
    pub issues: Vec<ValidationIssue>,
}

/// 一项可定位到节点或连线的工作流校验问题。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// 稳定的机器可读问题码。
    pub code: ValidationIssueCode,
    /// 面向用户展示的中文说明。
    pub message: String,
    /// 相关节点 ID；工作流级问题为空。
    pub node_id: Option<String>,
    /// 相关连线 ID；工作流级问题为空。
    pub edge_id: Option<String>,
    /// 问题所在作用域；工作流级问题为空。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_id: Option<String>,
    /// 从根作用域到问题作用域的稳定 ID 路径。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structure_path: Vec<String>,
}

macro_rules! validation_issue_codes {
    ($($variant:ident => $value:literal),+ $(,)?) => {
        /// 工作流校验器和注册节点共同使用的开放问题类别。
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum ValidationIssueCode {
            $(
                #[doc = concat!("内置稳定问题码 `", $value, "`。")]
                $variant,
            )+
            /// 注册节点拥有的命名空间化问题码。
            Custom(String),
        }

        impl ValidationIssueCode {
            /// 创建自定义节点拥有的稳定问题码；已知内置值会规范化为内置变体。
            pub fn custom(value: impl Into<String>) -> Self {
                let value = value.into();
                match value.as_str() {
                    $($value => Self::$variant,)+
                    _ => Self::Custom(value),
                }
            }

            /// 返回前端、日志和持久化协议使用的稳定字符串。
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $value,)+
                    Self::Custom(value) => value,
                }
            }
        }

        impl Serialize for ValidationIssueCode {
            fn serialize<SerializerType>(
                &self,
                serializer: SerializerType,
            ) -> Result<SerializerType::Ok, SerializerType::Error>
            where
                SerializerType: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for ValidationIssueCode {
            fn deserialize<DeserializerType>(
                deserializer: DeserializerType,
            ) -> Result<Self, DeserializerType::Error>
            where
                DeserializerType: Deserializer<'de>,
            {
                String::deserialize(deserializer).map(|value| Self::custom(value))
            }
        }
    };
}

validation_issue_codes! {
    UnsupportedSchemaVersion => "unsupported_schema_version",
    EmptyWorkflowName => "empty_workflow_name",
    InvalidWorkflowInputs => "invalid_workflow_inputs",
    InvalidVariables => "invalid_variables",
    EmptyNodeId => "empty_node_id",
    DuplicateNodeId => "duplicate_node_id",
    EmptyEdgeId => "empty_edge_id",
    DuplicateEdgeId => "duplicate_edge_id",
    InvalidStartCount => "invalid_start_count",
    InvalidEndCount => "invalid_end_count",
    UnknownEdgeEndpoint => "unknown_edge_endpoint",
    SelfLoop => "self_loop",
    InvalidNodeDegree => "invalid_node_degree",
    InvalidCondition => "invalid_condition",
    InvalidBranch => "invalid_branch",
    CycleDetected => "cycle_detected",
    UnreachableNode => "unreachable_node",
    NoPathToEnd => "no_path_to_end",
    EmptyLogMessage => "empty_log_message",
    InvalidDelay => "invalid_delay",
    InvalidObservationPolicy => "invalid_observation_policy",
    InvalidLoop => "invalid_loop",
    InvalidFailure => "invalid_failure",
    InvalidAqlQuery => "invalid_aql_query",
    InvalidApplicationSpec => "invalid_application_spec",
    InvalidBrowserSpec => "invalid_browser_spec",
    ApplicationPermissionDenied => "application_permission_denied",
    InvalidBackendPolicy => "invalid_backend_policy",
    InvalidTargetWaitPolicy => "invalid_target_wait_policy",
    InvalidExtract => "invalid_extract",
    InvalidDataFormat => "invalid_data_format",
    InvalidCommand => "invalid_command",
    CommandPermissionDenied => "command_permission_denied",
    InvalidValueReference => "invalid_value_reference",
    UndeclaredVariable => "undeclared_variable",
    InvalidExpression => "invalid_expression",
    InvalidOutputBinding => "invalid_output_binding",
    InvalidVariableAssignment => "invalid_variable_assignment",
    InvalidResourceReference => "invalid_resource_reference",
    ReferenceNotDominating => "reference_not_dominating",
    UnknownNodeType => "unknown_node_type",
    InvalidNodeDefinition => "invalid_node_definition",
    InvalidScope => "invalid_scope",
    InvalidScopeBoundary => "invalid_scope_boundary",
    InvalidNodeSize => "invalid_node_size",
    InvalidComponentDefinition => "invalid_component_definition",
    ComponentExpansionFailed => "component_expansion_failed",
}

/// 已通过 registry 编译、可直接进入执行热路径的工作流。
pub(crate) struct PreparedWorkflow {
    /// 原始 definition 只保留图、元数据和事件所需字段。
    pub(crate) definition: WorkflowDefinition,
    /// 每个节点 ID 对应的强类型冻结执行对象。
    pub(crate) nodes: HashMap<String, Arc<dyn PreparedNode>>,
    /// 所有 ValueExpr::Expression 在 prepare 阶段编译得到的共享 AST。
    pub(crate) value_plan: Arc<RuntimeValuePlan>,
    /// 把扁平执行节点映射回组件实例和内部画布。
    pub(crate) source_map: ComponentSourceMap,
}

/// 使用内置节点注册表校验 schema v10 工作流。
pub fn validate_workflow(workflow: &WorkflowDefinition) -> ValidationReport {
    let registry = builtin_nodes::registry(
        Arc::new(UnavailableActionDispatcher),
        Arc::new(crate::UnavailableObservationDispatcher),
        Arc::new(UnavailableApplicationSessionProvider),
        Arc::new(UnavailableBrowserSessionProvider),
    );
    validate_workflow_with_registry(workflow, &registry)
}

/// 使用随工作流提供的版本锁定组件目录完成展开和内置节点校验。
pub fn validate_workflow_with_components(
    workflow: &WorkflowDefinition,
    components: &[FlowComponentDefinition],
) -> ValidationReport {
    let component_registry = match ComponentRegistry::from_definitions(components.iter().cloned()) {
        Ok(registry) => registry,
        Err(error) => {
            return ValidationReport {
                valid: false,
                issues: vec![issue(
                    ValidationIssueCode::InvalidComponentDefinition,
                    error.to_string(),
                    None,
                    None,
                )],
            };
        }
    };
    let expanded = match expand_components(workflow.clone(), &component_registry) {
        Ok(expanded) => expanded,
        Err(error) => {
            return ValidationReport {
                valid: false,
                issues: vec![issue(
                    ValidationIssueCode::ComponentExpansionFailed,
                    error.message,
                    error.node_id,
                    None,
                )],
            };
        }
    };
    let mut report = validate_workflow(&expanded.definition);
    remap_component_issues(&mut report, &expanded.source_map);
    report
}

/// 使用宿主装配的开放节点注册表校验工作流。
pub fn validate_workflow_with_registry(
    workflow: &WorkflowDefinition,
    registry: &NodeTypeRegistry,
) -> ValidationReport {
    validate_and_compile(workflow, registry).2
}

/// 编译并校验工作流；成功结果保证执行阶段不再读取动态 payload。
pub(crate) fn prepare_workflow(
    workflow: WorkflowDefinition,
    registry: &NodeTypeRegistry,
    source_map: ComponentSourceMap,
) -> Result<PreparedWorkflow, ValidationReport> {
    let (nodes, value_plan, mut report) = validate_and_compile(&workflow, registry);
    if report.valid {
        Ok(PreparedWorkflow {
            definition: workflow,
            nodes,
            value_plan,
            source_map,
        })
    } else {
        remap_component_issues(&mut report, &source_map);
        Err(report)
    }
}

/// 把扁平内部节点的校验定位还原到最外层组件实例。
fn remap_component_issues(report: &mut ValidationReport, source_map: &ComponentSourceMap) {
    for issue in &mut report.issues {
        let Some(expanded_node_id) = issue.node_id.as_deref() else {
            continue;
        };
        let Some(path) = source_map.get(expanded_node_id) else {
            continue;
        };
        let Some(root) = path.first() else {
            continue;
        };
        issue.message = format!("组件内部节点 '{}': {}", expanded_node_id, issue.message,);
        issue.node_id = Some(root.instance_node_id.clone());
    }
}

/// 完成单次 registry decode，并在同一批强类型节点上运行所有校验。
fn validate_and_compile(
    workflow: &WorkflowDefinition,
    registry: &NodeTypeRegistry,
) -> (
    HashMap<String, Arc<dyn PreparedNode>>,
    Arc<RuntimeValuePlan>,
    ValidationReport,
) {
    let mut issues = Vec::new();
    validate_workflow_metadata(workflow, &mut issues);

    let mut prepared_nodes = HashMap::new();
    let mut node_ids = HashSet::new();
    for node in workflow
        .graph
        .scopes
        .iter()
        .flat_map(|scope| scope.nodes.iter())
    {
        if node.id.trim().is_empty() {
            issues.push(issue(
                ValidationIssueCode::EmptyNodeId,
                "节点 ID 不能为空",
                Some(node.id.clone()),
                None,
            ));
        }
        if !node_ids.insert(node.id.clone()) {
            issues.push(issue(
                ValidationIssueCode::DuplicateNodeId,
                format!("节点 ID '{}' 重复", node.id),
                Some(node.id.clone()),
                None,
            ));
        }
        if node
            .output_bindings
            .keys()
            .any(|output_name| output_name.trim().is_empty())
        {
            issues.push(issue(
                ValidationIssueCode::InvalidOutputBinding,
                "自定义输出名称不能为空",
                Some(node.id.clone()),
                None,
            ));
        }
        match registry.compile(node) {
            Ok(prepared) => {
                match prepared.flow() {
                    NodeFlow::Start
                    | NodeFlow::End
                    | NodeFlow::Linear
                    | NodeFlow::Branch { .. }
                    | NodeFlow::Loop { .. }
                    | NodeFlow::LoopEntry
                    | NodeFlow::LoopContinue
                    | NodeFlow::LoopComplete => {}
                }
                issues.extend(prepared.validate(&NodeValidationContext {
                    workflow,
                    node_id: &node.id,
                }));
                prepared_nodes.insert(node.id.clone(), prepared);
            }
            Err(NodeRegistryError::UnknownType { type_id }) => issues.push(issue(
                ValidationIssueCode::UnknownNodeType,
                format!("节点类型 '{}' 没有注册编译器", type_id.as_str()),
                Some(node.id.clone()),
                None,
            )),
            Err(NodeRegistryError::InvalidDefinition { source, .. }) => issues.push(issue(
                ValidationIssueCode::InvalidNodeDefinition,
                source.message,
                Some(node.id.clone()),
                None,
            )),
            Err(NodeRegistryError::DuplicateType { type_id }) => issues.push(issue(
                ValidationIssueCode::InvalidNodeDefinition,
                format!("节点类型 '{}' 在注册表中重复", type_id.as_str()),
                Some(node.id.clone()),
                None,
            )),
        }
    }
    validate_global_edge_ids(workflow, &mut issues);
    validate_scopes(workflow, &prepared_nodes, &mut issues);
    for scope in &workflow.graph.scopes {
        let local_node_ids = scope
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<HashSet<_>>();
        let graph = build_graph(scope, &local_node_ids, &mut issues);
        validate_node_degrees(scope, &prepared_nodes, &graph, &mut issues);
        let (entry_ids, end_ids) = scope_terminals(scope, &prepared_nodes);
        let optional_node_ids = optional_scope_boundaries(scope);
        validate_graph_shape(
            &local_node_ids,
            &entry_ids,
            &end_ids,
            &optional_node_ids,
            &graph,
            &mut issues,
        );
        if scope.id == workflow.graph.root_scope_id {
            validate_terminal_counts(scope, &prepared_nodes, &end_ids, &mut issues);
        }
    }
    validate_data_references(workflow, &prepared_nodes, &mut issues);

    let value_plan = compile_value_plan(workflow, &prepared_nodes, &mut issues);
    annotate_issue_locations(workflow, &mut issues);

    (
        prepared_nodes,
        value_plan,
        ValidationReport {
            valid: issues.is_empty(),
            issues,
        },
    )
}

/// 编译所有节点输入与通用输出映射中的高级表达式，并定位语法错误。
fn compile_value_plan(
    workflow: &WorkflowDefinition,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    issues: &mut Vec<ValidationIssue>,
) -> Arc<RuntimeValuePlan> {
    let mut builder = RuntimeValuePlanBuilder::default();
    for node in workflow
        .graph
        .scopes
        .iter()
        .flat_map(|scope| scope.nodes.iter())
    {
        if let Some(prepared) = prepared_nodes.get(&node.id) {
            for input in prepared.value_inputs() {
                if let Err(message) = builder.compile(input.expression) {
                    issues.push(issue(
                        ValidationIssueCode::InvalidExpression,
                        format!("表达式编译失败：{message}"),
                        Some(node.id.clone()),
                        None,
                    ));
                }
            }
        }
        for (output_name, expression) in &node.output_bindings {
            if let Err(message) = builder.compile(expression) {
                issues.push(issue(
                    ValidationIssueCode::InvalidExpression,
                    format!("输出 '{output_name}' 的表达式编译失败：{message}"),
                    Some(node.id.clone()),
                    None,
                ));
            }
        }
    }
    builder.finish()
}

/// 创建一项稳定且可定位的校验问题。
pub(crate) fn issue(
    code: ValidationIssueCode,
    message: impl Into<String>,
    node_id: Option<String>,
    edge_id: Option<String>,
) -> ValidationIssue {
    ValidationIssue {
        code,
        message: message.into(),
        node_id,
        edge_id,
        scope_id: None,
        structure_path: Vec::new(),
    }
}
