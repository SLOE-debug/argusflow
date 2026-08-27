use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use argusflow_core::WorkflowDefinition;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    NodeFlow, NodeRegistryError, NodeTypeRegistry, NodeValidationContext, PreparedNode,
    UnavailableActionDispatcher, UnavailableApplicationSessionProvider,
    UnavailableBrowserSessionProvider, builtin_nodes,
    validation_graph::{build_graph, validate_graph_shape, validate_node_degrees},
    validation_references::validate_data_references,
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
    InvalidAqlQuery => "invalid_aql_query",
    InvalidApplicationSpec => "invalid_application_spec",
    InvalidBrowserSpec => "invalid_browser_spec",
    ApplicationPermissionDenied => "application_permission_denied",
    InvalidBackendPolicy => "invalid_backend_policy",
    InvalidCommand => "invalid_command",
    CommandPermissionDenied => "command_permission_denied",
    InvalidValueReference => "invalid_value_reference",
    InvalidExpression => "invalid_expression",
    InvalidOutputBinding => "invalid_output_binding",
    InvalidVariableAssignment => "invalid_variable_assignment",
    InvalidResourceReference => "invalid_resource_reference",
    ReferenceNotDominating => "reference_not_dominating",
    UnknownNodeType => "unknown_node_type",
    InvalidNodeDefinition => "invalid_node_definition",
}

/// 已通过 registry 编译、可直接进入执行热路径的工作流。
pub(crate) struct PreparedWorkflow {
    /// 原始 definition 只保留图、元数据和事件所需字段。
    pub(crate) definition: WorkflowDefinition,
    /// 每个节点 ID 对应的强类型冻结执行对象。
    pub(crate) nodes: HashMap<String, Arc<dyn PreparedNode>>,
    /// 所有 ValueExpr::Expression 在 prepare 阶段编译得到的共享 AST。
    pub(crate) value_plan: Arc<RuntimeValuePlan>,
}

/// 使用内置节点注册表校验 schema v8 工作流。
pub fn validate_workflow(workflow: &WorkflowDefinition) -> ValidationReport {
    let registry = builtin_nodes::registry(
        Arc::new(UnavailableActionDispatcher),
        Arc::new(UnavailableApplicationSessionProvider),
        Arc::new(UnavailableBrowserSessionProvider),
    );
    validate_workflow_with_registry(workflow, &registry)
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
) -> Result<PreparedWorkflow, ValidationReport> {
    let (nodes, value_plan, report) = validate_and_compile(&workflow, registry);
    if report.valid {
        Ok(PreparedWorkflow {
            definition: workflow,
            nodes,
            value_plan,
        })
    } else {
        Err(report)
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
    let mut start_ids = Vec::new();
    let mut end_ids = Vec::new();
    for node in &workflow.nodes {
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
                    NodeFlow::Start => start_ids.push(node.id.clone()),
                    NodeFlow::End => end_ids.push(node.id.clone()),
                    NodeFlow::Linear | NodeFlow::Branch { .. } => {}
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
    validate_terminal_counts(&start_ids, &end_ids, &mut issues);

    let graph = build_graph(workflow, &node_ids, &mut issues);
    validate_node_degrees(workflow, &prepared_nodes, &graph, &mut issues);
    validate_graph_shape(&node_ids, &start_ids, &end_ids, &graph, &mut issues);
    if start_ids.len() == 1 {
        validate_data_references(
            workflow,
            &prepared_nodes,
            &start_ids[0],
            &graph.predecessors,
            &mut issues,
        );
    }

    let value_plan = compile_value_plan(workflow, &prepared_nodes, &mut issues);

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
    for node in &workflow.nodes {
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

/// 校验工作流级契约。
fn validate_workflow_metadata(workflow: &WorkflowDefinition, issues: &mut Vec<ValidationIssue>) {
    if workflow.schema_version != 8 {
        issues.push(issue(
            ValidationIssueCode::UnsupportedSchemaVersion,
            "schema_version 必须为 8",
            None,
            None,
        ));
    }
    if workflow.name.trim().is_empty() {
        issues.push(issue(
            ValidationIssueCode::EmptyWorkflowName,
            "工作流名称不能为空",
            None,
            None,
        ));
    }
    let mut input_keys = HashSet::new();
    for input in &workflow.inputs {
        if input.key.trim().is_empty() || !input_keys.insert(input.key.as_str()) {
            issues.push(issue(
                ValidationIssueCode::InvalidWorkflowInputs,
                "工作流输入名称必须非空且唯一",
                None,
                None,
            ));
        }
    }
    if !workflow.variables.is_object() {
        issues.push(issue(
            ValidationIssueCode::InvalidVariables,
            "工作流变量根值必须是 JSON 对象",
            None,
            None,
        ));
    }
}

/// 校验唯一 Start 与 End 数量。
fn validate_terminal_counts(
    start_ids: &[String],
    end_ids: &[String],
    issues: &mut Vec<ValidationIssue>,
) {
    if start_ids.len() != 1 {
        issues.push(issue(
            ValidationIssueCode::InvalidStartCount,
            "工作流必须且只能包含一个 Start 节点",
            None,
            None,
        ));
    }
    if end_ids.len() != 1 {
        issues.push(issue(
            ValidationIssueCode::InvalidEndCount,
            "工作流必须且只能包含一个 End 节点",
            None,
            None,
        ));
    }
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
    }
}
