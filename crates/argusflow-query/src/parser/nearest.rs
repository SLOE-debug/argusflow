//! `nearest` 空间选择器的具名参数解析。

use std::num::NonZeroUsize;

use argusflow_core::{DistanceMetric, QueryExpr, SpatialDirection};

use crate::{AqlError, AqlErrorKind, lexer::TokenKind};

use super::Parser;

impl Parser<'_> {
    /// 解析具名参数形式的 `nearest(anchor=..., target=..., direction=..., index=...)`。
    pub(super) fn parse_nearest(&mut self) -> Result<QueryExpr, AqlError> {
        self.expect_left_paren("nearest 后必须使用括号传入空间查询参数")?;
        self.expect_named_argument("anchor")?;
        let anchor = self.parse_relation()?;
        self.expect_comma("nearest anchor 后需要 target 参数")?;
        self.expect_named_argument("target")?;
        let target = self.parse_relation()?;
        self.expect_comma("nearest target 后需要 direction 参数")?;
        self.expect_named_argument("direction")?;
        let direction_token = self.current().clone();
        let TokenKind::Identifier(direction_name) = &direction_token.kind else {
            return Err(self.unexpected(
                &direction_token,
                "nearest direction 必须是 above、below、left、right 或 any",
            ));
        };
        let direction = parse_direction(direction_name).ok_or_else(|| {
            self.error(
                &direction_token,
                AqlErrorKind::InvalidArgument,
                format!("未知 nearest direction '{direction_name}'"),
                None,
            )
        })?;
        self.advance();
        self.expect_comma("nearest direction 后需要 index 参数")?;
        self.expect_named_argument("index")?;
        let index_token = self.current().clone();
        let TokenKind::Integer(index) = index_token.kind else {
            return Err(self.unexpected(&index_token, "nearest index 必须是从 1 开始的整数"));
        };
        let Some(index) = NonZeroUsize::new(index) else {
            return Err(self.error(
                &index_token,
                AqlErrorKind::InvalidArgument,
                "nearest index 必须大于 0",
                None,
            ));
        };
        self.advance();
        let mut metric = DistanceMetric::default();
        if matches!(self.current().kind, TokenKind::Comma) {
            self.advance();
            self.expect_named_argument("metric")?;
            let metric_token = self.current().clone();
            let TokenKind::Identifier(metric_name) = &metric_token.kind else {
                return Err(self.unexpected(
                    &metric_token,
                    "nearest metric 必须是 edge_gap 或 center_distance",
                ));
            };
            metric = parse_distance_metric(metric_name).ok_or_else(|| {
                self.error(
                    &metric_token,
                    AqlErrorKind::InvalidArgument,
                    format!("未知 nearest metric '{metric_name}'"),
                    None,
                )
            })?;
            self.advance();
        }
        self.expect_right_paren(
            "nearest 参数必须按 anchor、target、direction、index、metric 排列",
        )?;
        Ok(QueryExpr::Nearest {
            anchor: Box::new(anchor),
            target: Box::new(target),
            direction,
            index,
            metric,
        })
    }

    /// 要求并消费 `name =` 具名参数前缀。
    fn expect_named_argument(&mut self, expected: &str) -> Result<(), AqlError> {
        let name_token = self.current().clone();
        let TokenKind::Identifier(actual) = &name_token.kind else {
            return Err(self.unexpected(&name_token, &format!("此处需要具名参数 {expected}")));
        };
        if actual != expected {
            return Err(self.error(
                &name_token,
                AqlErrorKind::InvalidArgument,
                format!("此处需要具名参数 '{expected}'，实际为 '{actual}'"),
                None,
            ));
        }
        self.advance();
        if !matches!(self.current().kind, TokenKind::Equal) {
            return Err(self.unexpected(self.current(), "具名参数后必须使用 ="));
        }
        self.advance();
        Ok(())
    }
}

/// 将空间方向关键字映射为强类型枚举。
fn parse_direction(name: &str) -> Option<SpatialDirection> {
    Some(match name {
        "any" => SpatialDirection::Any,
        "above" => SpatialDirection::Above,
        "below" => SpatialDirection::Below,
        "left" => SpatialDirection::Left,
        "right" => SpatialDirection::Right,
        _ => return None,
    })
}

/// 将距离度量关键字映射为强类型枚举。
fn parse_distance_metric(name: &str) -> Option<DistanceMetric> {
    Some(match name {
        "edge_gap" => DistanceMetric::EdgeGapNormalized,
        "center_distance" => DistanceMetric::CenterDistanceNormalized,
        _ => return None,
    })
}
