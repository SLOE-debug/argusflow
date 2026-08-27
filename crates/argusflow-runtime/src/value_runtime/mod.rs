//! 统一的 JSON 值解析、受限表达式求值和 Published Outputs 映射边界。

mod evaluator;
mod formatter;
mod output_mapping;
mod scope;

use std::{collections::HashMap, sync::Arc};

use argusflow_core::ValueExpr;
use rhai::AST;

pub(crate) use formatter::format_runtime_value;
pub(crate) use output_mapping::publish_outcome;
pub(crate) use scope::RuntimeValueScope;

/// prepare 阶段建立、运行阶段只读共享的表达式 AST 集合。
#[derive(Debug, Default)]
pub(crate) struct RuntimeValuePlan {
    /// 表达式源码唯一决定其 AST，同源码可以跨字段安全复用。
    expressions: HashMap<String, Arc<AST>>,
}

impl RuntimeValuePlan {
    /// 创建没有高级表达式的空计划。
    pub(crate) fn empty() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 返回已经在 prepare 阶段编译的 AST。
    fn expression(&self, source: &str) -> Option<&AST> {
        self.expressions.get(source).map(AsRef::as_ref)
    }
}

/// 单次工作流 prepare 使用的表达式计划构造器。
#[derive(Debug, Default)]
pub(crate) struct RuntimeValuePlanBuilder {
    /// 只保存已经验证通过的表达式，失败不会留下半编译计划。
    expressions: HashMap<String, Arc<AST>>,
}

impl RuntimeValuePlanBuilder {
    /// 编译一个 ValueExpr；字面量和结构化引用不需要 AST。
    pub(crate) fn compile(&mut self, expression: &ValueExpr) -> Result<(), String> {
        let ValueExpr::Expression { source } = expression else {
            return Ok(());
        };
        if self.expressions.contains_key(source) {
            return Ok(());
        }
        let ast = evaluator::compile_expression(source)?;
        self.expressions.insert(source.clone(), Arc::new(ast));
        Ok(())
    }

    /// 冻结编译结果，供任意 RunWorld 只读共享。
    pub(crate) fn finish(self) -> Arc<RuntimeValuePlan> {
        Arc::new(RuntimeValuePlan {
            expressions: self.expressions,
        })
    }
}

/// 使用计划中的 AST 在冻结作用域上执行表达式。
pub(crate) fn evaluate_expression(
    plan: &RuntimeValuePlan,
    source: &str,
    scope: &RuntimeValueScope,
) -> Result<serde_json::Value, crate::RuntimeError> {
    let ast = plan.expression(source).ok_or_else(|| {
        crate::RuntimeError::ExecutionInvariant(format!(
            "expression was not compiled during workflow preparation: {source}"
        ))
    })?;
    evaluator::evaluate(ast, scope)
}

/// 检查持久化指针是否符合项目采用的 RFC 6901 起始与转义规则。
pub(crate) fn validate_json_pointer(pointer: &str) -> bool {
    if pointer.is_empty() {
        return true;
    }
    if !pointer.starts_with('/') {
        return false;
    }
    let bytes = pointer.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            if !matches!(bytes.get(index + 1).copied(), Some(b'0' | b'1')) {
                return false;
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use argusflow_core::ValueExpr;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::RuntimeValuePlanBuilder;
    use crate::{NodeOutcome, RunContext, RuntimeError};

    #[test]
    fn expressions_read_input_variables_and_prior_published_outputs() {
        let expression = ValueExpr::Expression {
            source: "input.order + \"/\" + vars.region + \"/\" + nodes[\"read\"].text".to_owned(),
        };
        let mut builder = RuntimeValuePlanBuilder::default();
        builder
            .compile(&expression)
            .expect("fixture expression should compile");
        let mut context = RunContext::with_value_plan(
            Uuid::new_v4(),
            json!({ "order": "10086" }).as_object().unwrap().clone(),
            json!({ "region": "east" }).as_object().unwrap().clone(),
            builder.finish(),
        );
        context.record_outcome(
            "read".to_owned(),
            NodeOutcome::values(BTreeMap::from([(
                "text".to_owned(),
                Value::String("ready".to_owned()),
            )])),
        );

        assert_eq!(
            context
                .resolve_value(&expression)
                .expect("all three scope roots should resolve"),
            json!("10086/east/ready")
        );
    }

    #[test]
    fn expressions_cannot_access_the_resource_plane() {
        let expression = ValueExpr::Expression {
            source: "resources".to_owned(),
        };
        let mut builder = RuntimeValuePlanBuilder::default();
        builder
            .compile(&expression)
            .expect("an unknown variable remains valid expression syntax");
        let context = RunContext::with_value_plan(
            Uuid::new_v4(),
            Default::default(),
            Default::default(),
            builder.finish(),
        );

        assert!(matches!(
            context.resolve_value(&expression),
            Err(RuntimeError::ExpressionEvaluation { .. })
        ));
    }
}
