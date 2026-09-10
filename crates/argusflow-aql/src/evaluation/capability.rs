//! 编译后执行前检查来源能力，禁止静默返回空结果。
use crate::{Attribute, BoundQuery, Boundary, Expr, Role};
use argusflow_core::{Failure, FailureKind};
use std::collections::BTreeSet;

/// 来源对角色、属性、结构和原生查询的明确支持。
#[derive(Debug, Clone)]
pub struct Capabilities {
    roles: BTreeSet<Role>,
    attributes: BTreeSet<Attribute>,
    relations: bool,
    css: bool,
    frame: bool,
    shadow: bool,
}
impl Capabilities {
    /// 声明真实支持的角色和属性；扩展能力默认关闭。
    pub fn new(
        roles: impl IntoIterator<Item = Role>,
        attributes: impl IntoIterator<Item = Attribute>,
    ) -> Self {
        Self {
            roles: roles.into_iter().collect(),
            attributes: attributes.into_iter().collect(),
            relations: false,
            css: false,
            frame: false,
            shadow: false,
        }
    }
    /// 显式开启元素树关系。
    pub fn with_relations(mut self) -> Self {
        self.relations = true;
        self
    }
    /// 显式开启浏览器 CSS 和文档边界。
    pub fn with_browser_boundaries(mut self) -> Self {
        self.css = true;
        self.frame = true;
        self.shadow = true;
        self
    }
    /// 在任何平台查询或图像识别之前检查整棵表达式。
    pub fn check(&self, query: &BoundQuery) -> Result<(), Failure> {
        if !query.attributes().is_subset(&self.attributes) {
            return Err(unsupported("来源不支持查询中使用的属性"));
        }
        self.check_expr(query.expression())
    }
    fn check_expr(&self, expression: &Expr) -> Result<(), Failure> {
        match expression {
            Expr::Match { role, .. } if *role != Role::Element && !self.roles.contains(role) => {
                return Err(unsupported("来源不支持查询中的角色"));
            }
            Expr::Relation { left, right, .. } => {
                if !self.relations {
                    return Err(unsupported("来源没有元素层级"));
                }
                self.check_expr(left)?;
                self.check_expr(right)?;
            }
            Expr::Nth { query, .. } => self.check_expr(query)?,
            Expr::Css(_) if !self.css => return Err(unsupported("该来源不支持 CSS")),
            Expr::Enter { host, boundary } => {
                if !match boundary {
                    Boundary::Frame => self.frame,
                    Boundary::Shadow => self.shadow,
                } {
                    return Err(unsupported("来源不支持文档边界"));
                }
                self.check_expr(host)?;
            }
            _ => {}
        }
        Ok(())
    }
}
fn unsupported(message: &str) -> Failure {
    Failure::new(FailureKind::Unsupported, "aql_capability", message)
}
