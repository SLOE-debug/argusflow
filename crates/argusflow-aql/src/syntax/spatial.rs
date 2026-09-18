//! 空间字段解析独立于属性条件，重复字段和矛盾规则一律拒绝。
use super::parser::Parser;
use crate::{
    AqlError, DiagnosticCode, Expr, Length, LengthUnit, MatchOperator, SpatialOptions,
    SpatialOrder, SpatialQuery, ValueType,
};
use std::collections::BTreeSet;

impl Parser<'_> {
    pub(super) fn spatial(&mut self) -> Result<Expr, AqlError> {
        let mut anchor = None;
        let mut target = None;
        let mut region = None;
        let mut second_anchor = None;
        let mut options = SpatialOptions::default();
        let mut fields = BTreeSet::new();
        loop {
            let field = self.word().to_owned();
            if !fields.insert(field.clone()) {
                return Err(self.error(DiagnosticCode::Syntax, "空间字段不能重复"));
            }
            self.advance();
            self.expect("=")?;
            match field.as_str() {
                "anchor" => anchor = Some(Box::new(self.relation()?)),
                "element" => target = Some(Box::new(self.relation()?)),
                "scope" => region = Some(Box::new(self.relation()?)),
                "second_anchor" => second_anchor = Some(Box::new(self.relation()?)),
                "direction" => {
                    options.angles = match self.word() {
                        "right" => Some((337.5, 22.5)),
                        "upper_right" => Some((22.5, 67.5)),
                        "up" => Some((67.5, 112.5)),
                        "upper_left" => Some((112.5, 157.5)),
                        "left" => Some((157.5, 202.5)),
                        "lower_left" => Some((202.5, 247.5)),
                        "down" => Some((247.5, 292.5)),
                        "lower_right" => Some((292.5, 337.5)),
                        "any_direction" => None,
                        _ => {
                            return Err(
                                self.error(DiagnosticCode::Type, "方向需要八方向之一或任意方向")
                            );
                        }
                    };
                    self.advance();
                }
                "angles" => {
                    let start = self.angle()?;
                    self.expect("degrees_to")?;
                    let end = self.angle()?;
                    self.expect("degrees")?;
                    if start == end {
                        return Err(self.error(
                            DiagnosticCode::Type,
                            "角度起点与终点不能相同；全方向请使用任意方向",
                        ));
                    }
                    options.angles = Some((start, end));
                }
                "distance_basis" => {
                    options.edge_distance = match self.word() {
                        "edge_distance" => true,
                        "center_distance" => false,
                        _ => {
                            return Err(
                                self.error(DiagnosticCode::Type, "距离依据需要中心距离或边缘距离")
                            );
                        }
                    };
                    self.advance();
                }
                "min_distance" => options.min_distance = Some(self.length()?),
                "max_distance" => options.max_distance = Some(self.length()?),
                "alignment" => {
                    options.row = Some(match self.word() {
                        "same_row" => true,
                        "same_column" => false,
                        _ => return Err(self.error(DiagnosticCode::Type, "对齐需要同一行或同一列")),
                    });
                    self.advance();
                }
                "tolerance" => options.tolerance = Some(self.length()?),
                "exclude_overlap" => {
                    options.exclude_overlap = self.boolean()?;
                }
                "region_mode" => {
                    options.outside = match self.word() {
                        "inside" => false,
                        "outside" => true,
                        _ => {
                            return Err(
                                self.error(DiagnosticCode::Type, "区域关系需要区域内或区域外")
                            );
                        }
                    };
                    self.advance();
                }
                "between_mode" => {
                    options.between_rectangle = match self.word() {
                        "rectangle" => true,
                        "segment" => false,
                        _ => {
                            return Err(
                                self.error(DiagnosticCode::Type, "两锚点模式需要线段或矩形")
                            );
                        }
                    };
                    self.advance();
                }
                "bandwidth" => options.bandwidth = Some(self.length()?),
                "sort" => options.order = Some(self.spatial_order()?),
                "secondary_sort" => options.secondary = Some(self.spatial_order()?),
                "select" => options.rank = Some(self.rank()?),
                _ => return Err(self.error(DiagnosticCode::UnknownSymbol, "未知空间字段")),
            }
            if !self.take(",") {
                break;
            }
        }
        if fields.contains("direction") && fields.contains("angles") {
            return Err(self.error(DiagnosticCode::Type, "方向与角度区间不能同时填写"));
        }
        if options.row.is_some() != options.tolerance.is_some() {
            return Err(self.error(DiagnosticCode::Type, "对齐与对齐容差需要一起填写"));
        }
        if fields.contains("region_mode") && region.is_none() {
            return Err(self.error(DiagnosticCode::Type, "区域关系需要明确范围查询"));
        }
        if second_anchor.is_none()
            && (options.bandwidth.is_some() || fields.contains("between_mode"))
        {
            return Err(self.error(DiagnosticCode::Type, "两锚点条件需要第二锚点"));
        }
        if second_anchor.is_some() && !options.between_rectangle && options.bandwidth.is_none() {
            return Err(self.error(DiagnosticCode::Type, "线段模式需要明确带宽"));
        }
        if options.between_rectangle && options.bandwidth.is_some() {
            return Err(self.error(DiagnosticCode::Type, "矩形模式不接受线段带宽"));
        }
        if options.rank.is_some() && options.order.is_none()
            || options.secondary.is_some() && options.order.is_none()
        {
            return Err(self.error(DiagnosticCode::Type, "选择序号或次排序需要明确排序依据"));
        }
        Ok(Expr::Spatial(Box::new(SpatialQuery {
            anchor: anchor.ok_or_else(|| self.error(DiagnosticCode::Syntax, "缺少锚点查询"))?,
            target: target.ok_or_else(|| self.error(DiagnosticCode::Syntax, "缺少目标查询"))?,
            region,
            second_anchor,
            options,
        })))
    }
    fn angle(&mut self) -> Result<f64, AqlError> {
        let number = self
            .word()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && (0.0..360.0).contains(v))
            .ok_or_else(|| {
                self.error(DiagnosticCode::Type, "角度必须在 0（含）到 360（不含）之间")
            })?;
        self.advance();
        Ok(number)
    }
    fn boolean(&mut self) -> Result<bool, AqlError> {
        let value = match self.word() {
            "true" => true,
            "false" => false,
            _ => return Err(self.error(DiagnosticCode::Type, "需要是或否")),
        };
        self.advance();
        Ok(value)
    }
    fn length(&mut self) -> Result<Length, AqlError> {
        let unit = match self.word() {
            "scope_short" => Some(LengthUnit::ScopeShort),
            "scope_width" => Some(LengthUnit::ScopeWidth),
            "scope_height" => Some(LengthUnit::ScopeHeight),
            _ => None,
        };
        if unit.is_some() {
            self.advance();
        }
        let value = self.operand(ValueType::Number, MatchOperator::Equal)?;
        if matches!(&value,crate::Operand::Literal(crate::Value::Number(v)) if !v.is_finite() || *v<0.0 || *v>1e12)
        {
            return Err(self.error(DiagnosticCode::Type, "空间长度必须为预算内的非负有限数"));
        }
        if unit.is_some() {
            self.expect("%")?;
        } else {
            self.expect("logical_pixels")?;
        }
        Ok(Length {
            value,
            unit: unit.unwrap_or(LengthUnit::LogicalPixels),
        })
    }
    pub(super) fn rank(&mut self) -> Result<usize, AqlError> {
        let word = self.word();
        let number = word
            .strip_prefix('第')
            .and_then(|w| w.strip_suffix('个'))
            .unwrap_or(word)
            .parse::<usize>()
            .ok()
            .filter(|v| *v > 0 && *v <= 4096)
            .ok_or_else(|| self.error(DiagnosticCode::Type, "选择需要第 1 到第 4096 个"))?;
        self.advance();
        Ok(number)
    }
    pub(super) fn spatial_order(&mut self) -> Result<SpatialOrder, AqlError> {
        let order = match self.word() {
            "near" => SpatialOrder::Near,
            "far" => SpatialOrder::Far,
            "top" => SpatialOrder::Top,
            "left_order" => SpatialOrder::Left,
            "top_left" => SpatialOrder::TopLeft,
            _ => return Err(self.error(DiagnosticCode::Type, "未知空间排序")),
        };
        self.advance();
        Ok(order)
    }
}
