use serde::{Deserialize, Deserializer, Serialize};

use crate::ValueExpr;

/// 相对于当前视觉视口的归一化矩形。
///
/// 所有坐标都必须位于 `[0, 1]` 内，且矩形不能越过视口边界。该类型在反序列化时
/// 立即建立不变量，避免无效区域进入视觉查询或缓存策略。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct NormalizedRect {
    /// 左侧起点，范围为 `[0, 1]`。
    x: f32,
    /// 顶部起点，范围为 `[0, 1]`。
    y: f32,
    /// 归一化宽度，必须大于零。
    width: f32,
    /// 归一化高度，必须大于零。
    height: f32,
}

impl NormalizedRect {
    /// 使用四个归一化分量创建有效区域。
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Result<Self, &'static str> {
        let rect = Self {
            x,
            y,
            width,
            height,
        };
        rect.is_valid()
            .then_some(rect)
            .ok_or("normalized rectangle must stay within a finite unit viewport")
    }

    /// 判断区域是否满足视觉视口不变量。
    pub fn is_valid(self) -> bool {
        self.x().is_finite()
            && self.y().is_finite()
            && self.width().is_finite()
            && self.height().is_finite()
            && self.x() >= 0.0
            && self.y() >= 0.0
            && self.width() > 0.0
            && self.height() > 0.0
            && self.x() + self.width() <= 1.0
            && self.y() + self.height() <= 1.0
    }

    /// 返回归一化区域的水平起点。
    pub const fn x(self) -> f32 {
        self.x
    }

    /// 返回归一化区域的垂直起点。
    pub const fn y(self) -> f32 {
        self.y
    }

    /// 返回归一化区域的宽度。
    pub const fn width(self) -> f32 {
        self.width
    }

    /// 返回归一化区域的高度。
    pub const fn height(self) -> f32 {
        self.height
    }
}

impl<'de> Deserialize<'de> for NormalizedRect {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireRect {
            x: f32,
            y: f32,
            width: f32,
            height: f32,
        }

        let rect = WireRect::deserialize(deserializer)?;
        Self::new(rect.x, rect.y, rect.width, rect.height).map_err(serde::de::Error::custom)
    }
}

/// Runtime 接收的已解析视觉查询。
///
/// `text` 已由 Runtime 从 `ValueExpr` 解析为本次尝试冻结的字符串；视觉后端不再读取
/// 工作流输入、变量或表达式环境。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualQuery {
    /// 本次执行要识别的文字。
    pub text: String,
    /// 是否要求识别文字完全相等。
    pub exact: bool,
    /// 可选的归一化识别区域。
    #[serde(default)]
    pub region: Option<NormalizedRect>,
}

/// Workflow 持久化的视觉查询表达式。
///
/// `text` 保持完整 `ValueExpr`，因此动态输入可以经过统一的引用、类型和支配关系
/// 校验。反序列化仍接受早期版本的字符串字段，并立即转换为字符串字面量表达式。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VisualQueryExpr {
    /// 运行时解析为目标文字的值表达式。
    pub text: ValueExpr,
    /// 是否要求识别文字完全相等。
    pub exact: bool,
    /// 可选的归一化识别区域。
    #[serde(default)]
    pub region: Option<NormalizedRect>,
}

impl<'de> Deserialize<'de> for VisualQueryExpr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum TextWire {
            /// 旧版视觉查询直接保存的字符串文字。
            Legacy(String),
            /// 当前版本保存的完整值表达式。
            Expression(ValueExpr),
        }

        #[derive(Deserialize)]
        struct WireQuery {
            text: TextWire,
            exact: bool,
            #[serde(default)]
            region: Option<NormalizedRect>,
        }

        let query = WireQuery::deserialize(deserializer)?;
        Ok(Self {
            text: match query.text {
                TextWire::Legacy(text) => ValueExpr::text(text),
                TextWire::Expression(expression) => expression,
            },
            exact: query.exact,
            region: query.region,
        })
    }
}
