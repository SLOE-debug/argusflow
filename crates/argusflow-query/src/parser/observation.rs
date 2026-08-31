//! AQL v3 顶层投影、聚合与布尔表达式解析。

use std::collections::BTreeSet;

use argusflow_core::{
    EntityField, NumberComparison, NumberOperand, ObservationExpr, ObservationValueType, QueryExpr,
    UiQuery,
};

use crate::{AqlError, AqlErrorKind, lexer::TokenKind};

use super::Parser;

impl Parser<'_> {
    /// 解析一个完整观察表达式，并在构造 AST 时检查组合器输入类型。
    pub(super) fn parse_observation_expression(&mut self) -> Result<ObservationExpr, AqlError> {
        let function = match &self.current().kind {
            TokenKind::Identifier(name) => Some(name.clone()),
            _ => None,
        };
        match function.as_deref() {
            Some("project") => self.parse_project(),
            Some("count") => self.parse_count_or_comparison(),
            Some("exists") => self.parse_exists(),
            Some("all_of") => self.parse_boolean_list("all_of"),
            Some("any_of") => self.parse_boolean_list("any_of"),
            Some("not") => self.parse_observation_not(),
            _ => self
                .parse_relation()
                .map(|query| ObservationExpr::Entities {
                    query: UiQuery::new(query),
                }),
        }
    }

    /// 解析 `project(selector, fields = [name, ...])`。
    fn parse_project(&mut self) -> Result<ObservationExpr, AqlError> {
        self.advance();
        self.expect_left_paren("project 后必须使用括号")?;
        let query = UiQuery::new(self.parse_relation()?);
        self.expect_comma("project 必须声明 fields")?;
        let fields_name = self.current().clone();
        if !matches!(&fields_name.kind, TokenKind::Identifier(name) if name == "fields") {
            return Err(self.unexpected(&fields_name, "project 第二个参数必须是 fields"));
        }
        self.advance();
        let equal = self.current().clone();
        if !matches!(equal.kind, TokenKind::Equal) {
            return Err(self.unexpected(&equal, "fields 后必须使用 ="));
        }
        self.advance();
        let left = self.current().clone();
        if !matches!(left.kind, TokenKind::LeftBracket) {
            return Err(self.unexpected(&left, "fields 必须使用方括号列出固定字段"));
        }
        self.advance();
        let mut fields = Vec::new();
        let mut unique = BTreeSet::new();
        loop {
            let field_token = self.current().clone();
            let TokenKind::Identifier(name) = &field_token.kind else {
                return Err(self.unexpected(&field_token, "fields 中需要固定实体字段名"));
            };
            let field = parse_entity_field(name).ok_or_else(|| {
                self.error(
                    &field_token,
                    AqlErrorKind::InvalidArgument,
                    format!("未知实体字段 '{name}'"),
                    Some(
                        "可用字段：name、text、value、role、bounds、confidence、source".to_owned(),
                    ),
                )
            })?;
            if !unique.insert(field) {
                return Err(self.error(
                    &field_token,
                    AqlErrorKind::InvalidArgument,
                    format!("实体字段 '{name}' 重复"),
                    None,
                ));
            }
            fields.push(field);
            self.advance();
            if !matches!(self.current().kind, TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        let right = self.current().clone();
        if !matches!(right.kind, TokenKind::RightBracket) {
            return Err(self.unexpected(&right, "fields 缺少结束方括号"));
        }
        self.advance();
        self.expect_right_paren("project 只接受 selector 与 fields")?;
        Ok(ObservationExpr::Project { query, fields })
    }

    /// 解析 `count(selector)` 以及可选的数值比较后缀。
    fn parse_count_or_comparison(&mut self) -> Result<ObservationExpr, AqlError> {
        self.advance();
        self.expect_left_paren("count 后必须使用括号传入 selector")?;
        let query = UiQuery::new(self.parse_relation()?);
        self.expect_right_paren("count 只接受一个 selector")?;
        let count = ObservationExpr::Count { query };
        let operator = match self.current().kind {
            TokenKind::Equal => Some(NumberComparison::Equal),
            TokenKind::NotEqual => Some(NumberComparison::NotEqual),
            TokenKind::Child => Some(NumberComparison::GreaterThan),
            TokenKind::GreaterThanOrEqual => Some(NumberComparison::GreaterThanOrEqual),
            TokenKind::LessThan => Some(NumberComparison::LessThan),
            TokenKind::LessThanOrEqual => Some(NumberComparison::LessThanOrEqual),
            _ => None,
        };
        let Some(operator) = operator else {
            return Ok(count);
        };
        self.advance();
        let right_token = self.current().clone();
        let right = match &right_token.kind {
            TokenKind::Integer(value) => {
                NumberOperand::Literal(u64::try_from(*value).map_err(|_| {
                    self.error(
                        &right_token,
                        AqlErrorKind::InvalidArgument,
                        "数量比较右值超出 u64 范围",
                        None,
                    )
                })?)
            }
            TokenKind::Parameter(name) => NumberOperand::Parameter(name.clone()),
            _ => return Err(self.unexpected(&right_token, "数量比较右值必须是非负整数或参数")),
        };
        self.advance();
        Ok(ObservationExpr::Compare {
            left: Box::new(count),
            operator,
            right,
        })
    }

    /// 解析存在性断言。
    fn parse_exists(&mut self) -> Result<ObservationExpr, AqlError> {
        self.advance();
        self.expect_left_paren("exists 后必须使用括号传入 selector")?;
        let query = UiQuery::new(self.parse_relation()?);
        self.expect_right_paren("exists 只接受一个 selector")?;
        Ok(ObservationExpr::Exists { query })
    }

    /// 解析至少两个布尔子表达式组成的强三值组合。
    fn parse_boolean_list(&mut self, name: &str) -> Result<ObservationExpr, AqlError> {
        self.advance();
        self.expect_left_paren(&format!("{name} 后必须使用括号"))?;
        let mut expressions = Vec::new();
        loop {
            let expression = self.parse_observation_expression()?;
            self.require_boolean(&expression, name)?;
            expressions.push(expression);
            if !matches!(self.current().kind, TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        self.expect_right_paren(&format!("{name} 参数之间必须使用逗号"))?;
        if expressions.len() < 2 {
            return Err(AqlError::at(
                self.source,
                0,
                self.source.len(),
                AqlErrorKind::InvalidArgument,
                format!("{name} 至少需要两个布尔表达式"),
                None,
            ));
        }
        Ok(if name == "all_of" {
            ObservationExpr::AllOf { expressions }
        } else {
            ObservationExpr::AnyOf { expressions }
        })
    }

    /// 根据内部类型区分选择器 `not` 与布尔 `not`。
    fn parse_observation_not(&mut self) -> Result<ObservationExpr, AqlError> {
        self.advance();
        self.expect_left_paren("not 后必须使用括号")?;
        let expression = self.parse_observation_expression()?;
        self.expect_right_paren("not 只接受一个表达式")?;
        match expression {
            ObservationExpr::Entities { query } => Ok(ObservationExpr::Entities {
                query: UiQuery::new(QueryExpr::Not {
                    query: Box::new(query.expression),
                }),
            }),
            boolean if boolean.value_type() == ObservationValueType::Boolean => {
                Ok(ObservationExpr::Not {
                    expression: Box::new(boolean),
                })
            }
            other => Err(AqlError::at(
                self.source,
                0,
                self.source.len(),
                AqlErrorKind::InvalidArgument,
                format!("not 不支持 {:?} 结果", other.value_type()),
                None,
            )),
        }
    }

    /// 在组合器边界拒绝实体、记录和数量值。
    fn require_boolean(&self, expression: &ObservationExpr, owner: &str) -> Result<(), AqlError> {
        if expression.value_type() == ObservationValueType::Boolean {
            return Ok(());
        }
        Err(AqlError::at(
            self.source,
            0,
            self.source.len(),
            AqlErrorKind::InvalidArgument,
            format!("{owner} 只接受布尔表达式"),
            Some("请使用 exists(selector) 或 count(selector) >= n".to_owned()),
        ))
    }
}

/// 将固定字段语法映射为核心枚举。
fn parse_entity_field(name: &str) -> Option<EntityField> {
    Some(match name {
        "name" => EntityField::Name,
        "text" => EntityField::Text,
        "value" => EntityField::Value,
        "role" => EntityField::Role,
        "bounds" => EntityField::Bounds,
        "confidence" => EntityField::Confidence,
        "source" => EntityField::Source,
        _ => return None,
    })
}
