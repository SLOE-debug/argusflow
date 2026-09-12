//! 每个节点的唯一后继与结构分支形成有界的结构化图。
use super::{prepare::Compiler, *};
use argusflow_workflow::{Diagnostic, DiagnosticCode, EdgeEndpoint, Fields, ScopeGraph, ValueType};
use std::collections::{BTreeMap, BTreeSet};

impl Compiler<'_> {
    pub fn scope(
        &mut self,
        scope: usize,
        mut env: Environment,
        injected: Vec<(String, ValueType)>,
    ) -> Result<(), Diagnostic> {
        let saved = self.position.clone();
        self.position = (scope, None);
        if env.chain.len() >= 64 || !self.claimed.insert(scope) {
            return Err(self.error(
                DiagnosticCode::Structure,
                "作用域重复拥有、循环拥有或嵌套超过 64 层",
            ));
        }
        self.declare(scope, injected.clone())?;
        env.chain.push(scope);
        for (name, _) in injected {
            env.initialized.insert(self.symbols[scope][&name]);
        }
        let definition = self.workflow.scopes[scope].clone();
        let by_id = definition
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<BTreeMap<_, _>>();
        let mut visited = BTreeSet::new();
        let graph = ScopeGraph::new(&definition)?;
        let mut cursor = graph.successor(&EdgeEndpoint::Start)?;
        let mut nodes = Vec::new();
        let mut terminal = false;
        while let EdgeEndpoint::Node { node: id } = cursor {
            self.position = (scope, Some(id.to_owned()));
            if terminal || !visited.insert(id) {
                return Err(self.error(DiagnosticCode::Structure, "环或终止节点之后存在连线"));
            }
            let node = by_id
                .get(id.as_str())
                .ok_or_else(|| self.error(DiagnosticCode::Structure, "后继必须属于当前作用域"))?;
            let (action, native_types) = self.action(&node.action, scope, &mut env)?;
            self.position = (scope, Some(id.to_owned()));
            terminal = !self.can_complete(&action);
            if terminal && !node.output_bindings.is_empty() {
                return Err(self.error(DiagnosticCode::Control, "控制转移节点不能发布输出映射"));
            }
            let mut output_types = native_types.clone();
            let mut mappings = BTreeMap::new();
            for (name, expr) in &node.output_bindings {
                if output_types.contains_key(name) {
                    return Err(self.error(DiagnosticCode::Type, "输出映射不能覆盖原生输出"));
                }
                let compiled = self.expr(expr, &env, &native_types)?;
                output_types.insert(name.clone(), compiled.ty.clone());
                mappings.insert(name.clone(), compiled);
            }
            self.validate_fields(&output_types)?;
            env.outputs
                .insert(id.to_owned(), (scope, nodes.len(), output_types.clone()));
            nodes.push(PlanNode {
                id: id.to_owned(),
                timeout_ms: node.timeout_ms,
                action,
                mappings,
                output_types,
            });
            let source = EdgeEndpoint::node(id);
            cursor = if terminal {
                graph
                    .outgoing(&source)
                    .first()
                    .map(|edge| &edge.target)
                    .unwrap_or(&EdgeEndpoint::End)
            } else {
                graph.successor(&source)?
            };
        }
        if visited.len() != definition.nodes.len() {
            return Err(self.error(DiagnosticCode::Structure, "存在不可达节点或入口缺失"));
        }
        self.position = (scope, None);
        let outputs = if terminal {
            if !definition.outputs.is_empty() {
                return Err(self.error(DiagnosticCode::Control, "终止路径不能声明正常出口表达式"));
            }
            BTreeMap::new()
        } else {
            definition
                .outputs
                .iter()
                .map(|(name, expression)| {
                    Ok((name.clone(), self.expr(expression, &env, &Fields::new())?))
                })
                .collect::<Result<_, Diagnostic>>()?
        };
        self.plans[scope] = Some(PlanScope {
            can_complete: !terminal,
            id: definition.id,
            nodes,
            slots: self.slots[scope].clone(),
            outputs,
        });
        self.position = saved;
        Ok(())
    }
    pub fn child(
        &mut self,
        name: &str,
        env: &Environment,
        injected: Vec<(String, ValueType)>,
    ) -> Result<usize, Diagnostic> {
        let id = self.lookup_scope(name)?;
        self.scope(id, env.clone(), injected)?;
        Ok(id)
    }
    pub fn resource_arguments(
        &self,
        arguments: &BTreeMap<String, String>,
        fields: &BTreeMap<String, String>,
        env: &Environment,
    ) -> Result<BTreeMap<String, ResourceBinding>, Diagnostic> {
        if arguments.keys().ne(fields.keys()) {
            return Err(self.error(DiagnosticCode::Resource, "资源参数缺失或多余"));
        }
        arguments
            .iter()
            .map(|(port, name)| {
                let (binding, ty, _) = env.resources.get(name).ok_or_else(|| {
                    self.error(DiagnosticCode::Resource, format!("资源尚不可用：{name}"))
                })?;
                if ty != &fields[port] {
                    return Err(self.error(DiagnosticCode::Resource, "资源类型不匹配"));
                }
                Ok((port.clone(), binding.clone()))
            })
            .collect()
    }
}
