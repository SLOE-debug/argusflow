//! 内置控制结构的词法与端口检查。
use super::{prepare::Compiler, *};
use argusflow_workflow::{Action, Diagnostic, DiagnosticCode as Code, Fields, ValueType as Ty};
use std::collections::BTreeSet;

impl Compiler<'_> {
    pub fn action(
        &mut self,
        action: &Action,
        scope: usize,
        env: &mut Environment,
    ) -> Result<(PlanAction, Fields), Diagnostic> {
        let mut outputs = Fields::new();
        let plan = match action {
            Action::Let {
                name,
                value_type,
                value,
            } => {
                let binding = self.symbols[scope][name];
                let value = self.typed(value, value_type, env)?;
                env.initialized.insert(binding);
                PlanAction::Let {
                    slot: binding.slot,
                    value,
                }
            }
            Action::Assign { assignments } => {
                if assignments.is_empty() {
                    return Err(self.error(Code::Reference, "赋值不能为空"));
                }
                let mut targets = BTreeSet::new();
                let mut compiled = Vec::new();
                for assignment in assignments {
                    let binding = env
                        .chain
                        .iter()
                        .rev()
                        .find_map(|scope| self.symbols[*scope].get(&assignment.name))
                        .copied()
                        .ok_or_else(|| self.error(Code::Reference, "赋值目标未声明"))?;
                    if !env.initialized.contains(&binding) {
                        return Err(self.error(Code::Uninitialized, "赋值目标仍处于暂时性死区"));
                    }
                    if !self.slots[binding.scope][binding.slot].mutable || !targets.insert(binding)
                    {
                        return Err(self.error(Code::Reference, "不能修改只读变量或重复赋值目标"));
                    }
                    if binding.scope == self.root {
                        self.active_reads.insert(binding);
                    }
                    let ty = self.slots[binding.scope][binding.slot].ty.clone();
                    compiled.push((binding, self.typed(&assignment.value, &ty, env)?));
                }
                PlanAction::Assign(compiled)
            }
            Action::Block { scope: child } => {
                let child = self.child(child, env, Vec::new())?;
                outputs = self.output_types(child);
                PlanAction::Block(child)
            }
            Action::If {
                condition,
                then_scope,
                else_scope,
            } => {
                let condition = self.typed(condition, &Ty::Bool, env)?;
                let then_scope = self.child(then_scope, env, Vec::new())?;
                let else_scope = self.child(else_scope, env, Vec::new())?;
                outputs = self.merge_outputs(&[then_scope, else_scope])?;
                PlanAction::If {
                    condition,
                    then_scope,
                    else_scope,
                }
            }
            Action::Switch {
                selector,
                cases,
                default_scope,
            } => {
                let selector = self.expr(selector, env, &Fields::new())?;
                if !matches!(selector.ty, Ty::Bool | Ty::Int | Ty::Text) {
                    return Err(self.error(Code::Type, "Switch 选择器必须为布尔、整数或文字"));
                }
                let mut compiled = Vec::new();
                for case in cases {
                    if !case.value.matches(&selector.ty)
                        || compiled.iter().any(|(value, _)| value == &case.value)
                    {
                        return Err(self.error(Code::Type, "Switch 分支重复或类型不匹配"));
                    }
                    compiled.push((
                        case.value.clone(),
                        self.child(&case.scope, env, Vec::new())?,
                    ));
                }
                let default_scope = self.child(default_scope, env, Vec::new())?;
                let scopes = compiled
                    .iter()
                    .map(|(_, scope)| *scope)
                    .chain([default_scope])
                    .collect::<Vec<_>>();
                outputs = self.merge_outputs(&scopes)?;
                PlanAction::Switch {
                    selector,
                    cases: compiled,
                    default_scope,
                }
            }
            Action::While {
                condition,
                body,
                max_iterations,
            } => {
                self.iteration_limit(*max_iterations)?;
                let condition = self.typed(condition, &Ty::Bool, env)?;
                let mut child_env = env.clone();
                child_env.loop_depth += 1;
                let body = self.child(body, &child_env, Vec::new())?;
                self.no_outputs(body)?;
                PlanAction::While {
                    condition,
                    body,
                    max_iterations: *max_iterations,
                }
            }
            Action::ForEach {
                items,
                item,
                index,
                body,
                max_iterations,
            } => {
                self.iteration_limit(*max_iterations)?;
                let items = self.expr(items, env, &Fields::new())?;
                let Ty::List(ty) = &items.ty else {
                    return Err(self.error(Code::Type, "ForEach 需要列表"));
                };
                let mut child_env = env.clone();
                child_env.loop_depth += 1;
                let body = self.child(
                    body,
                    &child_env,
                    vec![(item.clone(), *ty.clone()), (index.clone(), Ty::Int)],
                )?;
                self.no_outputs(body)?;
                PlanAction::ForEach {
                    item_slot: self.symbols[body][item].slot,
                    index_slot: self.symbols[body][index].slot,
                    items,
                    body,
                    max_iterations: *max_iterations,
                }
            }
            Action::Break | Action::Continue => {
                if env.loop_depth == 0 {
                    return Err(self.error(
                        Code::Control,
                        "Break/Continue 必须位于本次调用的循环内，且不能离开 Finally",
                    ));
                }
                if matches!(action, Action::Break) {
                    PlanAction::Break
                } else {
                    PlanAction::Continue
                }
            }
            Action::Call {
                subflow,
                inputs,
                resources,
            } => {
                let saved = self.position.clone();
                let child = self.subflow(subflow)?;
                self.position = saved;
                for binding in &self.requirements[subflow] {
                    if !env.initialized.contains(binding) {
                        return Err(
                            self.error(Code::Uninitialized, "子流程依赖的全局变量尚未初始化")
                        );
                    }
                    self.active_reads.insert(*binding);
                }
                let definition = &self.workflow.subflows[subflow];
                outputs = definition.outputs.clone();
                let input_types = definition.inputs.clone();
                let resource_types = definition.resources.clone();
                PlanAction::Call {
                    scope: child,
                    inputs: self.arguments(inputs, &input_types, env)?,
                    resources: self.resource_arguments(resources, &resource_types, env)?,
                }
            }
            Action::Return { values } => {
                if env.in_finally {
                    return Err(self.error(Code::Control, "Finally 不能用 Return 覆盖原始控制转移"));
                }
                let expected = env
                    .return_types
                    .clone()
                    .ok_or_else(|| self.error(Code::Control, "缺少调用返回契约"))?;
                PlanAction::Return(self.arguments(values, &expected, env)?)
            }
            Action::Try {
                body,
                catches,
                finally,
            } => {
                let body = self.child(body, env, Vec::new())?;
                let mut compiled = Vec::new();
                let mut kinds = BTreeSet::new();
                for catch in catches {
                    if catch.errors.is_empty()
                        || catch
                            .errors
                            .iter()
                            .any(|kind| !kind.catchable() || !kinds.insert(*kind))
                    {
                        return Err(self.error(Code::Control, "Catch 类型为空、重复或不可捕获"));
                    }
                    let ty = Ty::Record(Fields::from([
                        ("kind".into(), Ty::Text),
                        ("code".into(), Ty::Text),
                    ]));
                    let scope =
                        self.child(&catch.scope, env, vec![(catch.error_name.clone(), ty)])?;
                    compiled.push(PlanCatch {
                        errors: catch.errors.clone(),
                        scope,
                        error_slot: self.symbols[scope][&catch.error_name].slot,
                    });
                }
                let scopes = [body]
                    .into_iter()
                    .chain(compiled.iter().map(|catch| catch.scope))
                    .collect::<Vec<_>>();
                outputs = self.merge_outputs(&scopes)?;
                let finally = if let Some(name) = finally {
                    let mut child_env = env.clone();
                    child_env.in_finally = true;
                    child_env.loop_depth = 0;
                    let scope = self.child(name, &child_env, Vec::new())?;
                    self.no_outputs(scope)?;
                    Some(scope)
                } else {
                    None
                };
                PlanAction::Try {
                    body,
                    catches: compiled,
                    finally,
                }
            }
            Action::Fail { code } => {
                if !super::prepare::valid_name(code) {
                    return Err(self.error(Code::Reference, "错误代码不能为空或过长"));
                }
                PlanAction::Fail(code.clone())
            }
            Action::Wait { milliseconds } => {
                PlanAction::Wait(self.typed(milliseconds, &Ty::Int, env)?)
            }
            Action::Task { task } => {
                let (compiled, signature) = self
                    .registry
                    .compile(task)
                    .map_err(|msg| self.error(Code::Task, msg))?;
                self.validate_fields(&signature.inputs)?;
                self.validate_fields(&signature.outputs)?;
                self.validate_resources(&signature.resources)?;
                self.validate_resources(&signature.resource_outputs)?;
                if let Some(retry) = &task.retry
                    && (!signature.safe_to_retry
                        || !signature.resource_outputs.is_empty()
                        || retry.max_attempts == 0
                        || retry.max_attempts > 10
                        || retry.initial_delay_ms > retry.max_delay_ms
                        || retry.max_delay_ms > 60_000
                        || retry.errors.is_empty()
                        || retry.errors.iter().any(|kind| !kind.catchable()))
                {
                    return Err(self.error(Code::Task, "重试策略超限或任务不允许安全重试"));
                }
                let inputs = self.arguments(&task.inputs, &signature.inputs, env)?;
                let resources =
                    self.resource_arguments(&task.resources, &signature.resources, env)?;
                if task
                    .resource_outputs
                    .keys()
                    .ne(signature.resource_outputs.keys())
                {
                    return Err(self.error(Code::Resource, "资源输出端口缺失或多余"));
                }
                for (port, name) in &task.resource_outputs {
                    if !super::prepare::valid_name(name)
                        || env
                            .resources
                            .get(name)
                            .is_some_and(|(binding, _, _)| binding.scope == scope)
                    {
                        return Err(self.error(Code::Resource, "当前作用域资源重复声明"));
                    }
                    env.resources.insert(
                        name.clone(),
                        (
                            ResourceBinding {
                                scope,
                                name: name.clone(),
                            },
                            signature.resource_outputs[port].clone(),
                            true,
                        ),
                    );
                }
                outputs = signature.outputs.clone();
                PlanAction::Task(PlanTask {
                    task: compiled,
                    signature,
                    inputs,
                    resources,
                    resource_outputs: task.resource_outputs.clone(),
                    retry: task.retry.clone(),
                })
            }
            Action::Release { resource } => {
                let (binding, _, owned) = env
                    .resources
                    .get(resource)
                    .cloned()
                    .ok_or_else(|| self.error(Code::Resource, "释放目标不存在"))?;
                if binding.scope != scope || !owned {
                    return Err(self.error(Code::Resource, "只能显式释放当前作用域自有资源"));
                }
                env.resources.remove(resource);
                PlanAction::Release(binding)
            }
        };
        Ok((plan, outputs))
    }
    fn iteration_limit(&self, count: Option<u32>) -> Result<(), Diagnostic> {
        if count.is_some_and(|n| n == 0 || n > 100_000) {
            return Err(self.error(Code::Limit, "循环预算必须在 1..=100000"));
        }
        Ok(())
    }
    fn no_outputs(&self, scope: usize) -> Result<(), Diagnostic> {
        if !self.output_types(scope).is_empty() {
            return Err(self.error(
                Code::Type,
                "循环体和 Finally 不发布输出，累计值应声明在循环外",
            ));
        }
        Ok(())
    }
    fn merge_outputs(&self, scopes: &[usize]) -> Result<Fields, Diagnostic> {
        let mut result = None;
        for scope in scopes {
            if self.plans[*scope].as_ref().is_some_and(|p| p.can_complete) {
                let fields = self.output_types(*scope);
                if result.as_ref().is_some_and(|expected| expected != &fields) {
                    return Err(self.error(Code::Type, "各正常分支的输出类型必须一致"));
                }
                result = Some(fields);
            }
        }
        Ok(result.unwrap_or_default())
    }
    pub fn can_complete(&self, action: &PlanAction) -> bool {
        let completes = |scope: usize| self.plans[scope].as_ref().is_some_and(|p| p.can_complete);
        match action {
            PlanAction::Return(_)
            | PlanAction::Fail(_)
            | PlanAction::Break
            | PlanAction::Continue => false,
            PlanAction::Block(scope) => completes(*scope),
            PlanAction::If {
                then_scope,
                else_scope,
                ..
            } => completes(*then_scope) || completes(*else_scope),
            PlanAction::Switch {
                cases,
                default_scope,
                ..
            } => completes(*default_scope) || cases.iter().any(|(_, scope)| completes(*scope)),
            PlanAction::Try {
                body,
                catches,
                finally,
            } => {
                (completes(*body) || catches.iter().any(|catch| completes(catch.scope)))
                    && finally.is_none_or(completes)
            }
            _ => true,
        }
    }
}
