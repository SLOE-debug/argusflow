use std::num::NonZeroUsize;

use argusflow_core::{
    AqlQuery, DomAttribute, ElementMatcher, ElementRole, MatchOperator, PredicateValue,
    PropertyPredicate, QueryExpr, QueryLanguageVersion, QueryParameter, QueryValueType,
    RegexLiteral, SelectorAttribute, UiQuery, UiaAttribute,
};
use regex::RegexBuilder;

use crate::{
    AqlError, AqlErrorKind,
    lexer::{Token, TokenKind, lex},
};

mod nearest;

/// 按持久化查询携带的语言版本解析 AQL。
pub fn parse_stored_query(query: &AqlQuery) -> Result<UiQuery, AqlError> {
    match query.language_version {
        QueryLanguageVersion::V1 | QueryLanguageVersion::V2 => parse_query(&query.source),
    }
}

/// 将 AQL v1 源码解析为强类型 AST，并完成谓词类型检查。
pub fn parse_query(source: &str) -> Result<UiQuery, AqlError> {
    if source.trim().is_empty() {
        return Err(AqlError::at(
            source,
            0,
            0,
            AqlErrorKind::EmptyQuery,
            "AQL 查询不能为空",
            Some("例如：button(name = \"保存\")".to_owned()),
        ));
    }

    let tokens = lex(source)?;
    let mut parser = Parser {
        source,
        tokens,
        position: 0,
    };
    let expression = parser.parse_relation()?;
    parser.expect_end()?;
    Ok(UiQuery::new(expression))
}

/// 对 token 序列执行递归下降解析。
struct Parser<'source> {
    /// 完整输入，用于构造诊断。
    source: &'source str,
    /// 预先完成词法检查的 token。
    tokens: Vec<Token>,
    /// 当前 token 索引。
    position: usize,
}

impl Parser<'_> {
    /// 解析左结合的 `>` 与 `>>` 关系链。
    fn parse_relation(&mut self) -> Result<QueryExpr, AqlError> {
        let mut expression = self.parse_primary()?;

        loop {
            let relation = match &self.current().kind {
                TokenKind::Child => Some(false),
                TokenKind::Descendant => Some(true),
                _ => None,
            };
            let Some(descendant) = relation else {
                break;
            };
            self.advance();
            let target = self.parse_primary()?;
            expression = if descendant {
                QueryExpr::Descendant {
                    ancestor: Box::new(expression),
                    target: Box::new(target),
                }
            } else {
                QueryExpr::Child {
                    parent: Box::new(expression),
                    target: Box::new(target),
                }
            };
        }

        Ok(expression)
    }

    /// 根据调用名称区分角色 matcher、组合器与 CSS escape hatch。
    fn parse_primary(&mut self) -> Result<QueryExpr, AqlError> {
        let token = self.current().clone();
        let TokenKind::Identifier(identifier) = &token.kind else {
            return Err(self.unexpected(&token, "此处需要元素角色或查询函数"));
        };
        self.advance();

        match identifier.as_str() {
            "css" => self.parse_css(),
            "any" => self.parse_any(),
            "not" => self.parse_unary_query("not"),
            "first" => self.parse_unary_query("first"),
            "nth" => self.parse_nth(),
            "nearest" => self.parse_nearest(),
            role => self.parse_matcher(role, &token),
        }
    }

    /// 解析 `css("...")`，CSS 内容保持不透明。
    fn parse_css(&mut self) -> Result<QueryExpr, AqlError> {
        self.expect_left_paren("css 后必须使用括号传入 selector")?;
        let selector_token = self.current().clone();
        let TokenKind::String(selector) = &selector_token.kind else {
            return Err(self.unexpected(&selector_token, "css(...) 只接受一个字符串参数"));
        };
        if selector.trim().is_empty() {
            return Err(self.error(
                &selector_token,
                AqlErrorKind::InvalidArgument,
                "CSS selector 不能为空",
                None,
            ));
        }
        self.advance();
        self.expect_right_paren("css(...) 只接受一个字符串参数")?;
        Ok(QueryExpr::Css {
            selector: selector.clone(),
        })
    }

    /// 解析至少包含两个有序分支的 `any(...)`。
    fn parse_any(&mut self) -> Result<QueryExpr, AqlError> {
        self.expect_left_paren("any 后必须使用括号传入查询")?;
        let mut queries = Vec::new();

        if matches!(&self.current().kind, TokenKind::RightParen) {
            return Err(self.error(
                self.current(),
                AqlErrorKind::InvalidArgument,
                "any(...) 至少需要两个查询分支",
                None,
            ));
        }

        loop {
            queries.push(self.parse_relation()?);
            if !matches!(&self.current().kind, TokenKind::Comma) {
                break;
            }
            self.advance();
            if matches!(&self.current().kind, TokenKind::RightParen) {
                return Err(self.unexpected(self.current(), "any(...) 不允许尾随逗号"));
            }
        }
        self.expect_right_paren("any(...) 的查询分支之间必须使用逗号")?;

        if queries.len() < 2 {
            return Err(AqlError::at(
                self.source,
                0,
                self.source.len(),
                AqlErrorKind::InvalidArgument,
                "any(...) 至少需要两个查询分支",
                Some("只有一个查询时请直接使用该查询".to_owned()),
            ));
        }
        Ok(QueryExpr::Any { queries })
    }

    /// 解析 `not(query)` 或 `first(query)`。
    fn parse_unary_query(&mut self, name: &str) -> Result<QueryExpr, AqlError> {
        self.expect_left_paren(&format!("{name} 后必须使用括号传入查询"))?;
        let query = self.parse_relation()?;
        self.expect_right_paren(&format!("{name}(...) 只接受一个查询参数"))?;

        Ok(match name {
            "not" => QueryExpr::Not {
                query: Box::new(query),
            },
            "first" => QueryExpr::First {
                query: Box::new(query),
            },
            _ => unreachable!("parser only calls known unary combinators"),
        })
    }

    /// 解析从一开始计数的 `nth(query, index)`。
    fn parse_nth(&mut self) -> Result<QueryExpr, AqlError> {
        self.expect_left_paren("nth 后必须使用括号传入查询和索引")?;
        let query = self.parse_relation()?;
        self.expect_comma("nth(...) 的查询与索引之间必须使用逗号")?;

        let index_token = self.current().clone();
        let TokenKind::Integer(index) = &index_token.kind else {
            return Err(self.unexpected(&index_token, "nth 索引必须是从 1 开始的整数"));
        };
        let Some(index) = NonZeroUsize::new(*index) else {
            return Err(self.error(
                &index_token,
                AqlErrorKind::InvalidArgument,
                "nth 索引必须大于 0",
                Some("第一个结果使用 nth(query, 1) 或 first(query)".to_owned()),
            ));
        };
        self.advance();
        self.expect_right_paren("nth(...) 只接受查询和索引两个参数")?;
        Ok(QueryExpr::Nth {
            query: Box::new(query),
            index,
        })
    }

    /// 解析 `role(predicate, ...)` 并建立强类型 matcher。
    fn parse_matcher(
        &mut self,
        role_name: &str,
        role_token: &Token,
    ) -> Result<QueryExpr, AqlError> {
        let role = parse_role(role_name).ok_or_else(|| {
            self.error(
                role_token,
                AqlErrorKind::UnknownRole,
                format!("未知元素角色 '{role_name}'"),
                Some("AQL v1 角色示例：button、textbox、window、dialog、text".to_owned()),
            )
        })?;
        self.expect_left_paren("元素角色后必须使用括号，例如 button()")?;

        let mut predicates = Vec::new();
        if !matches!(&self.current().kind, TokenKind::RightParen) {
            loop {
                predicates.push(self.parse_predicate()?);
                if !matches!(&self.current().kind, TokenKind::Comma) {
                    break;
                }
                self.advance();
                if matches!(&self.current().kind, TokenKind::RightParen) {
                    return Err(self.unexpected(self.current(), "属性列表不允许尾随逗号"));
                }
            }
        }
        self.expect_right_paren("元素属性之间必须使用逗号")?;

        Ok(QueryExpr::Match {
            matcher: ElementMatcher { role, predicates },
        })
    }

    /// 解析并检查一个属性谓词。
    fn parse_predicate(&mut self) -> Result<PropertyPredicate, AqlError> {
        let attribute_token = self.current().clone();
        let TokenKind::Identifier(attribute_name) = &attribute_token.kind else {
            return Err(self.unexpected(&attribute_token, "此处需要 AQL 属性名"));
        };
        let attribute = parse_attribute(attribute_name).ok_or_else(|| {
            self.error(
                &attribute_token,
                AqlErrorKind::UnknownProperty,
                format!("未知 AQL 属性 '{attribute_name}'"),
                Some(
                    "portable 属性：name、key、value、enabled、visible、focused、checked、selected"
                        .to_owned(),
                ),
            )
        })?;
        self.advance();

        let operator_token = self.current().clone();
        let operator = self.parse_operator(&operator_token)?;
        self.advance();
        let value = self.parse_predicate_value(attribute, operator)?;

        Ok(PropertyPredicate {
            attribute,
            operator,
            value,
        })
    }

    /// 将符号或关键词映射为明确运算符。
    fn parse_operator(&self, token: &Token) -> Result<MatchOperator, AqlError> {
        let operator = match &token.kind {
            TokenKind::Equal => MatchOperator::Equal,
            TokenKind::NotEqual => MatchOperator::NotEqual,
            TokenKind::Identifier(name) if name == "contains" => MatchOperator::Contains,
            TokenKind::Identifier(name) if name == "starts_with" => MatchOperator::StartsWith,
            TokenKind::Identifier(name) if name == "ends_with" => MatchOperator::EndsWith,
            TokenKind::Identifier(name) if name == "matches" => MatchOperator::Regex,
            TokenKind::Identifier(name) => {
                return Err(self.error(
                    token,
                    AqlErrorKind::UnknownOperator,
                    format!("未知 AQL 运算符 '{name}'"),
                    Some("可用运算符：=、!=、contains、starts_with、ends_with、matches".to_owned()),
                ));
            }
            _ => return Err(self.unexpected(token, "属性名后需要 AQL 运算符")),
        };
        Ok(operator)
    }

    /// 根据属性类型和运算符解析右值。
    fn parse_predicate_value(
        &mut self,
        attribute: SelectorAttribute,
        operator: MatchOperator,
    ) -> Result<PredicateValue, AqlError> {
        let value_token = self.current().clone();

        if attribute.is_boolean() {
            if !matches!(operator, MatchOperator::Equal | MatchOperator::NotEqual) {
                return Err(self.error(
                    &value_token,
                    AqlErrorKind::InvalidPredicate,
                    format!("布尔属性 '{attribute}' 只支持 = 和 != 运算符"),
                    None,
                ));
            }
            let value = match &value_token.kind {
                TokenKind::True => true,
                TokenKind::False => false,
                _ => {
                    return Err(self.unexpected(
                        &value_token,
                        &format!("布尔属性 '{attribute}' 的右值必须是 true 或 false"),
                    ));
                }
            };
            self.advance();
            return Ok(PredicateValue::Boolean(value));
        }

        if operator == MatchOperator::Regex {
            let TokenKind::Regex {
                pattern,
                case_insensitive,
            } = &value_token.kind
            else {
                return Err(self.unexpected(
                    &value_token,
                    "matches 的右值必须是 /pattern/ 或 /pattern/i 正则字面量",
                ));
            };
            validate_regex(pattern, *case_insensitive).map_err(|message| {
                self.error(&value_token, AqlErrorKind::InvalidRegex, message, None)
            })?;
            self.advance();
            return Ok(PredicateValue::Regex(RegexLiteral {
                pattern: pattern.clone(),
                case_insensitive: *case_insensitive,
            }));
        }

        match &value_token.kind {
            TokenKind::String(text) => {
                self.advance();
                Ok(PredicateValue::Text(text.clone()))
            }
            TokenKind::Parameter(name) => {
                self.advance();
                Ok(PredicateValue::Parameter(QueryParameter {
                    name: name.clone(),
                    expected_type: QueryValueType::Text,
                }))
            }
            _ => Err(self.unexpected(
                &value_token,
                "文本属性的右值必须是双引号字符串或 $parameter",
            )),
        }
    }

    /// 要求并消费左括号。
    fn expect_left_paren(&mut self, message: &str) -> Result<(), AqlError> {
        if !matches!(&self.current().kind, TokenKind::LeftParen) {
            return Err(self.unexpected(self.current(), message));
        }
        self.advance();
        Ok(())
    }

    /// 要求并消费右括号。
    fn expect_right_paren(&mut self, message: &str) -> Result<(), AqlError> {
        if !matches!(&self.current().kind, TokenKind::RightParen) {
            return Err(self.unexpected(self.current(), message));
        }
        self.advance();
        Ok(())
    }

    /// 要求并消费逗号。
    fn expect_comma(&mut self, message: &str) -> Result<(), AqlError> {
        if !matches!(&self.current().kind, TokenKind::Comma) {
            return Err(self.unexpected(self.current(), message));
        }
        self.advance();
        Ok(())
    }

    /// 确认根表达式后没有额外输入。
    fn expect_end(&self) -> Result<(), AqlError> {
        if matches!(&self.current().kind, TokenKind::End) {
            Ok(())
        } else {
            Err(self.unexpected(self.current(), "查询结束后存在多余内容"))
        }
    }

    /// 返回当前 token。
    fn current(&self) -> &Token {
        &self.tokens[self.position]
    }

    /// 消费当前 token；EOF 始终保留为最后一个 token。
    fn advance(&mut self) {
        if self.position + 1 < self.tokens.len() {
            self.position += 1;
        }
    }

    /// 构造通用的 unexpected-token 诊断。
    fn unexpected(&self, token: &Token, message: &str) -> AqlError {
        self.error(token, AqlErrorKind::UnexpectedToken, message, None)
    }

    /// 使用 token 范围构造诊断。
    fn error(
        &self,
        token: &Token,
        kind: AqlErrorKind,
        message: impl Into<String>,
        help: Option<String>,
    ) -> AqlError {
        AqlError::at(self.source, token.start, token.end, kind, message, help)
    }
}

/// 将 v1 角色关键字映射为领域枚举。
fn parse_role(name: &str) -> Option<ElementRole> {
    Some(match name {
        "window" => ElementRole::Window,
        "dialog" => ElementRole::Dialog,
        "pane" => ElementRole::Pane,
        "button" => ElementRole::Button,
        "textbox" => ElementRole::TextBox,
        "checkbox" => ElementRole::CheckBox,
        "radio" => ElementRole::Radio,
        "combobox" => ElementRole::ComboBox,
        "list" => ElementRole::List,
        "list_item" => ElementRole::ListItem,
        "tree" => ElementRole::Tree,
        "tree_item" => ElementRole::TreeItem,
        "tab" => ElementRole::Tab,
        "tab_item" => ElementRole::TabItem,
        "menu" => ElementRole::Menu,
        "menu_item" => ElementRole::MenuItem,
        "link" => ElementRole::Link,
        "image" => ElementRole::Image,
        "table" => ElementRole::Table,
        "row" => ElementRole::Row,
        "cell" => ElementRole::Cell,
        "document" => ElementRole::Document,
        "text" => ElementRole::Text,
        _ => return None,
    })
}

/// 将属性关键字映射为 portable 或显式 namespace 属性。
fn parse_attribute(name: &str) -> Option<SelectorAttribute> {
    Some(match name {
        "name" => SelectorAttribute::Name,
        "key" => SelectorAttribute::Key,
        "value" => SelectorAttribute::Value,
        "enabled" => SelectorAttribute::Enabled,
        "visible" => SelectorAttribute::Visible,
        "focused" => SelectorAttribute::Focused,
        "checked" => SelectorAttribute::Checked,
        "selected" => SelectorAttribute::Selected,
        "uia.automation_id" => SelectorAttribute::Uia(UiaAttribute::AutomationId),
        "uia.class_name" => SelectorAttribute::Uia(UiaAttribute::ClassName),
        "uia.accelerator_key" => SelectorAttribute::Uia(UiaAttribute::AcceleratorKey),
        "uia.access_key" => SelectorAttribute::Uia(UiaAttribute::AccessKey),
        "uia.framework_id" => SelectorAttribute::Uia(UiaAttribute::FrameworkId),
        "dom.test_id" => SelectorAttribute::Dom(DomAttribute::TestId),
        "dom.class" => SelectorAttribute::Dom(DomAttribute::Class),
        _ => return None,
    })
}

/// 提前编译正则，避免无效模式进入 AST 和后端规划阶段。
fn validate_regex(pattern: &str, case_insensitive: bool) -> Result<(), String> {
    RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .build()
        .map(|_| ())
        .map_err(|error| format!("正则表达式无效：{error}"))
}
