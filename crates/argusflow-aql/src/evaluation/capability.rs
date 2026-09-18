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
        let attributes = query.attributes();
        let missing = attributes
            .difference(&self.attributes)
            .map(|attribute| crate::localize(attribute.name()).source().to_owned())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(unsupported(&format!(
                "来源无法确认以下属性：{}",
                missing.join("、")
            )));
        }
        self.check_expr(query.expression())
    }
    fn check_expr(&self, expression: &Expr) -> Result<(), Failure> {
        match expression {
            Expr::Match { role, .. } if *role != Role::Element && !self.roles.contains(role) => {
                return Err(unsupported(&format!(
                    "来源无法确认目标类型：{}",
                    crate::localize(role.name()).source()
                )));
            }
            Expr::Relation { left, right, .. } => {
                if !self.relations {
                    return Err(unsupported("来源没有元素层级"));
                }
                self.check_expr(left)?;
                self.check_expr(right)?;
            }
            Expr::Nth { query, .. } | Expr::Position { query, .. } => self.check_expr(query)?,
            Expr::Spatial(spatial) => {
                self.check_expr(&spatial.anchor)?;
                self.check_expr(&spatial.target)?;
                for inner in [&spatial.region, &spatial.second_anchor]
                    .into_iter()
                    .flatten()
                {
                    self.check_expr(inner)?;
                }
            }
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
