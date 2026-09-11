//! 编译入口及跨调用的全局依赖分析。
use super::*;
use crate::{
    NodeRegistry,
    expression::{PlanExpr, compile_expr},
};
use argusflow_workflow::{Action, Diagnostic, DiagnosticCode, Expr, Fields, ValueType, Workflow};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// 校验并冻结文档；拒绝无效图、越界引用、未初始化变量和未知任务。
pub fn prepare(
    workflow: Workflow,
    registry: &NodeRegistry,
) -> Result<Arc<PreparedWorkflow>, Vec<Diagnostic>> {
    compile(workflow, registry, &BTreeMap::new())
        .map(Arc::new)
        .map_err(|error| vec![error])
}

pub(super) fn compile(
    workflow: Workflow,
    registry: &NodeRegistry,
    externals: &BTreeMap<argusflow_workflow::WorkflowId, Arc<PreparedWorkflow>>,
) -> Result<PreparedWorkflow, Diagnostic> {
    let general = |message: &str| Diagnostic {
        workflow: None,
        code: DiagnosticCode::Structure,
        message: message.into(),
        scope: None,
        node: None,
    };
    if workflow.schema_version != 1 || workflow.name.trim().is_empty() {
        return Err(general("文档版本必须为 1 且名称非空"));
    }
    if workflow.scopes.is_empty()
        || workflow.scopes.len() > 1024
        || workflow.scopes.iter().map(|s| s.nodes.len()).sum::<usize>() > 10_000
    {
        return Err(general("作用域或节点数量超出文档预算"));
    }
    let mut scope_ids = BTreeMap::new();
    let mut node_ids = BTreeSet::new();
    for (index, scope) in workflow.scopes.iter().enumerate() {
        if !valid_name(&scope.id) || scope_ids.insert(scope.id.clone(), index).is_some() {
            return Err(general("作用域 ID 为空、过长或重复"));
        }
        for node in &scope.nodes {
            let at_node = |message: &str| Diagnostic {
                scope: Some(scope.id.clone()),
                node: Some(node.id.clone()),
                ..general(message)
            };
            if !valid_name(&node.id) || !node_ids.insert(&node.id) {
                return Err(at_node("节点 ID 为空、过长或重复"));
            }
            if node.timeout_ms.is_some_and(|n| n == 0 || n > 86_400_000) {
                return Err(at_node("节点时限必须在 1..=86400000 毫秒"));
            }
        }
    }
    let root = *scope_ids
        .get(&workflow.root)
        .ok_or_else(|| general("根作用域不存在"))?;
    let count = workflow.scopes.len();
    let mut compiler = Compiler {
        workflow: &workflow,
        registry,
        externals,
        root,
        scope_ids,
        symbols: vec![BTreeMap::new(); count],
        slots: vec![Vec::new(); count],
        plans: (0..count).map(|_| None).collect(),
        claimed: BTreeSet::new(),
        compiling_calls: BTreeSet::new(),
        requirements: BTreeMap::new(),
        active_reads: BTreeSet::new(),
        position: (root, None),
    };
    compiler.declare(root, Vec::new())?;
    compiler.validate_fields(&workflow.inputs)?;
    compiler.validate_fields(&workflow.outputs)?;
    compiler.validate_resources(&workflow.resources)?;
    for name in workflow.subflows.keys() {
        compiler.subflow(name)?;
    }
    let mut env = Environment {
        inputs: workflow.inputs.clone(),
        return_types: Some(workflow.outputs.clone()),
        ..Environment::default()
    };
    for (name, ty) in &workflow.resources {
        if !valid_name(name) || !valid_name(ty) {
            return Err(general("资源名称或类型无效"));
        }
        env.resources.insert(
            name.clone(),
            (
                ResourceBinding {
                    scope: root,
                    name: name.clone(),
                },
                ty.clone(),
                false,
            ),
        );
    }
    compiler.scope(root, env, Vec::new())?;
    compiler.check_outputs(root, &workflow.outputs)?;
    if compiler.claimed.len() != count {
        return Err(general("存在未被根流程、结构节点或子流程拥有的作用域"));
    }
    let scopes = compiler
        .plans
        .into_iter()
        .map(|plan| plan.ok_or_else(|| general("作用域未编译")))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedWorkflow {
        identity: None,
        root,
        scopes,
        inputs: workflow.inputs,
        resource_inputs: workflow.resources,
        name: workflow.name,
        output_fields: workflow.outputs,
    })
}

pub(super) struct Compiler<'a> {
    pub externals: &'a BTreeMap<argusflow_workflow::WorkflowId, Arc<PreparedWorkflow>>,
    pub workflow: &'a Workflow,
    pub registry: &'a NodeRegistry,
    pub root: usize,
    pub scope_ids: BTreeMap<String, usize>,
    pub symbols: Vec<BTreeMap<String, Binding>>,
    pub slots: Vec<Vec<Slot>>,
    pub plans: Vec<Option<PlanScope>>,
    pub claimed: BTreeSet<usize>,
    pub compiling_calls: BTreeSet<String>,
    pub requirements: BTreeMap<String, BTreeSet<Binding>>,
    pub active_reads: BTreeSet<Binding>,
    pub position: (usize, Option<String>),
}

pub(super) fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 256
}

impl Compiler<'_> {
    pub fn error(&self, code: DiagnosticCode, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            workflow: None,
            code,
            message: message.into(),
            scope: Some(self.workflow.scopes[self.position.0].id.clone()),
            node: self.position.1.clone(),
        }
    }
    pub fn lookup_scope(&self, name: &str) -> Result<usize, Diagnostic> {
        self.scope_ids
            .get(name)
            .copied()
            .ok_or_else(|| self.error(DiagnosticCode::Structure, format!("作用域不存在：{name}")))
    }
    pub fn validate_fields(&self, fields: &Fields) -> Result<(), Diagnostic> {
        if fields.len() > 1024
            || !fields
                .iter()
                .all(|(name, ty)| valid_name(name) && ty.is_valid())
        {
            return Err(self.error(DiagnosticCode::Type, "类型结构或字段名称超过预算"));
        }
        Ok(())
    }
    pub fn validate_resources(&self, fields: &BTreeMap<String, String>) -> Result<(), Diagnostic> {
        if fields.len() > 1024
            || fields
                .iter()
                .any(|(name, ty)| !valid_name(name) || !valid_name(ty))
        {
            return Err(self.error(DiagnosticCode::Resource, "资源端口数量、名称或类型无效"));
        }
        Ok(())
    }
    pub fn declare(
        &mut self,
        scope: usize,
        injected: Vec<(String, ValueType)>,
    ) -> Result<(), Diagnostic> {
        if !self.symbols[scope].is_empty() {
            return Ok(());
        }
        let declarations = injected
            .into_iter()
            .map(|(name, ty)| (name, ty, false))
            .chain(
                self.workflow.scopes[scope]
                    .nodes
                    .iter()
                    .filter_map(|n| match &n.action {
                        Action::Let {
                            name, value_type, ..
                        } => Some((name.clone(), value_type.clone(), true)),
                        _ => None,
                    }),
            )
            .collect::<Vec<_>>();
        for (name, ty, mutable) in declarations {
            self.validate_fields(&Fields::from([(name.clone(), ty.clone())]))?;
            let binding = Binding {
                scope,
                slot: self.slots[scope].len(),
            };
            if self.symbols[scope].insert(name, binding).is_some() {
                return Err(self.error(DiagnosticCode::Reference, "同一作用域重复声明变量或参数"));
            }
            self.slots[scope].push(Slot { ty, mutable });
        }
        Ok(())
    }
    pub fn expr(
        &mut self,
        expression: &Expr,
        env: &Environment,
        result: &Fields,
    ) -> Result<PlanExpr, Diagnostic> {
        let (plan, reads) = compile_expr(expression, env, &self.symbols, &self.slots, result)
            .map_err(|(code, msg)| self.error(code, msg))?;
        self.active_reads.extend(
            reads
                .into_iter()
                .filter(|binding| binding.scope == self.root),
        );
        Ok(plan)
    }
    pub fn typed(
        &mut self,
        expression: &Expr,
        ty: &ValueType,
        env: &Environment,
    ) -> Result<PlanExpr, Diagnostic> {
        let plan = self.expr(expression, env, &Fields::new())?;
        if &plan.ty != ty {
            return Err(self.error(
                DiagnosticCode::Type,
                format!("表达式类型不匹配，要求 {ty:?}"),
            ));
        }
        Ok(plan)
    }
    pub fn arguments(
        &mut self,
        arguments: &BTreeMap<String, Expr>,
        fields: &Fields,
        env: &Environment,
    ) -> Result<BTreeMap<String, PlanExpr>, Diagnostic> {
        if arguments.keys().ne(fields.keys()) {
            return Err(self.error(DiagnosticCode::Type, "参数或返回字段缺失或多余"));
        }
        arguments
            .iter()
            .map(|(name, expr)| Ok((name.clone(), self.typed(expr, &fields[name], env)?)))
            .collect()
    }
    pub fn output_types(&self, scope: usize) -> Fields {
        self.plans[scope]
            .as_ref()
            .map(|scope| {
                scope
                    .outputs
                    .iter()
                    .map(|(name, e)| (name.clone(), e.ty.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn check_outputs(&self, scope: usize, expected: &Fields) -> Result<(), Diagnostic> {
        // 没有正常出口的作用域只通过 Return 返回，Return 已逐点检查。
        let plan = self.plans[scope]
            .as_ref()
            .ok_or_else(|| self.error(DiagnosticCode::Structure, "作用域未编译"))?;
        if plan.can_complete && &self.output_types(scope) != expected {
            return Err(self.error(DiagnosticCode::Type, "作用域公开输出与声明不一致"));
        }
        Ok(())
    }
    pub fn subflow(&mut self, name: &str) -> Result<usize, Diagnostic> {
        let definition = self
            .workflow
            .subflows
            .get(name)
            .cloned()
            .ok_or_else(|| self.error(DiagnosticCode::Reference, "子流程不存在"))?;
        let scope = self.lookup_scope(&definition.scope)?;
        if self.requirements.contains_key(name) {
            return Ok(scope);
        }
        if self.compiling_calls.len() >= 64 || !self.compiling_calls.insert(name.to_owned()) {
            return Err(self.error(DiagnosticCode::Control, "子流程递归调用或嵌套超过 64 层"));
        }
        if scope == self.root {
            return Err(self.error(DiagnosticCode::Structure, "根作用域不能同时是子流程"));
        }
        self.validate_fields(&definition.inputs)?;
        self.validate_fields(&definition.outputs)?;
        self.validate_resources(&definition.resources)?;
        let parent_reads = std::mem::take(&mut self.active_reads);
        let mut env = Environment {
            chain: vec![self.root],
            initialized: self.symbols[self.root].values().copied().collect(),
            inputs: definition.inputs,
            return_types: Some(definition.outputs.clone()),
            ..Environment::default()
        };
        for (name, ty) in definition.resources {
            env.resources
                .insert(name.clone(), (ResourceBinding { scope, name }, ty, false));
        }
        self.scope(scope, env, Vec::new())?;
        self.check_outputs(scope, &definition.outputs)?;
        let reads = std::mem::replace(&mut self.active_reads, parent_reads);
        self.requirements.insert(name.to_owned(), reads);
        self.compiling_calls.remove(name);
        Ok(scope)
    }
}
