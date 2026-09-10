//! 属性条件优先级与类型检查。
use super::parser::Parser;
use crate::{
    AqlError, Attribute, Condition, DiagnosticCode, MatchOperator, Operand, TokenKind, Value,
    ValueType,
};

impl Parser<'_> {
    pub(super) fn condition(&mut self) -> Result<Condition, AqlError> {
        self.enter()?;
        let mut left = self.conjunction()?;
        while self.take("or") {
            self.enter()?;
            left = Condition::Or(Box::new(left), Box::new(self.conjunction()?));
            self.leave();
        }
        self.leave();
        Ok(left)
    }
    fn conjunction(&mut self) -> Result<Condition, AqlError> {
        let mut left = self.unary()?;
        while self.take("and") || self.take(",") {
            self.enter()?;
            left = Condition::And(Box::new(left), Box::new(self.unary()?));
            self.leave();
        }
        Ok(left)
    }
    fn unary(&mut self) -> Result<Condition, AqlError> {
        self.enter()?;
        let result = if self.take("not") {
            Condition::Not(Box::new(self.unary()?))
        } else if self.take("(") {
            let condition = self.condition()?;
            self.expect(")")?;
            condition
        } else {
            self.comparison()?
        };
        self.leave();
        Ok(result)
    }
    fn comparison(&mut self) -> Result<Condition, AqlError> {
        let attribute = Attribute::parse(self.word())
            .ok_or_else(|| self.error(DiagnosticCode::UnknownSymbol, "未知 AQL 属性"))?;
        self.advance();
        let operator = match self.word() {
            "=" => MatchOperator::Equal,
            "!=" => MatchOperator::NotEqual,
            "contains" => MatchOperator::Contains,
            "starts_with" => MatchOperator::StartsWith,
            "ends_with" => MatchOperator::EndsWith,
            "matches" => MatchOperator::Regex,
            ">" => MatchOperator::Greater,
            ">=" => MatchOperator::GreaterEqual,
            "<" => MatchOperator::Less,
            "<=" => MatchOperator::LessEqual,
            _ => return Err(self.error(DiagnosticCode::Syntax, "属性后需要比较运算符")),
        };
        let expected = attribute.value_type();
        if !crate::checking::accepts(operator, expected) {
            return Err(self.error(DiagnosticCode::Type, "运算符与属性类型不匹配"));
        }
        self.advance();
        let operand = self.operand(expected, operator)?;
        Ok(Condition::Compare {
            attribute,
            operator,
            operand,
        })
    }
    fn operand(
        &mut self,
        expected: ValueType,
        operator: MatchOperator,
    ) -> Result<Operand, AqlError> {
        if operator == MatchOperator::Regex {
            if !self.current().is_some_and(|t| t.kind == TokenKind::Regex) {
                return Err(
                    self.error(DiagnosticCode::Type, "matches 右侧必须是 /模式/ 或 /模式/i")
                );
            }
            let raw = self.word();
            let end = raw
                .rfind('/')
                .ok_or_else(|| self.error(DiagnosticCode::Regex, "缺少正则结束符"))?;
            let insensitive = match &raw[end + 1..] {
                "" => false,
                "i" => true,
                _ => return Err(self.error(DiagnosticCode::Regex, "正则只支持 i 标志")),
            };
            let pattern = raw[1..end].replace("\\/", "/");
            let compiled = regex::RegexBuilder::new(&pattern)
                .case_insensitive(insensitive)
                .size_limit(1_048_576)
                .build()
                .map_err(|_| self.error(DiagnosticCode::Regex, "正则表达式无效或超过编译预算"))?;
            self.advance();
            return Ok(Operand::Regex {
                pattern,
                insensitive,
                compiled,
            });
        }
        if self
            .current()
            .is_some_and(|t| t.kind == TokenKind::Parameter)
        {
            let name = self.word()[1..].to_owned();
            if let Some(previous) = self.parameters.insert(name.clone(), expected)
                && previous != expected
            {
                return Err(self.error(DiagnosticCode::Type, "同一参数被用于不同类型的属性"));
            }
            self.advance();
            return Ok(Operand::Parameter(name));
        }
        let value = match expected {
            ValueType::Text if self.current().is_some_and(|t| t.kind == TokenKind::String) => {
                Value::Text(
                    serde_json::from_str(self.word())
                        .map_err(|_| self.error(DiagnosticCode::Lexical, "字符串转义无效"))?,
                )
            }
            ValueType::Boolean if matches!(self.word(), "true" | "false") => {
                Value::Boolean(self.word() == "true")
            }
            ValueType::Number => {
                let number: f64 = self
                    .word()
                    .parse()
                    .map_err(|_| self.error(DiagnosticCode::Type, "需要有限数值"))?;
                if !number.is_finite() {
                    return Err(self.error(DiagnosticCode::Type, "需要有限数值"));
                }
                Value::Number(number)
            }
            _ => return Err(self.error(DiagnosticCode::Type, "右值与属性类型不一致")),
        };
        self.advance();
        Ok(Operand::Literal(value))
    }
}
