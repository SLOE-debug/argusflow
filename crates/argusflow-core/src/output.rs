use std::collections::BTreeMap;

use serde_json::Value;

use crate::{AutomationAction, UiOperation};

/// 自动化读取动作对外暴露的稳定输出字段名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionOutputKey {
    /// 面向用户显示的文本。
    Text,
    /// 控件值接口返回的值。
    Value,
    /// 唯一目标的结构化投影。
    Item,
    /// 多目标的结构化投影集合。
    Items,
    /// 链接结构化集合。
    Links,
}

impl ActionOutputKey {
    /// 返回跨语言协议中使用的稳定字段名。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Value => "value",
            Self::Item => "item",
            Self::Items => "items",
            Self::Links => "links",
        }
    }
}

/// 一次动作允许产生的输出字段集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionOutputContract {
    /// 动作不产生输出。
    None,
    /// 产生一个文本字段。
    Text,
    /// 产生一个控件值字段。
    Value,
    /// 产生一个唯一目标字段。
    Item,
    /// 产生多个目标字段。
    Items,
    /// 产生链接集合字段。
    Links,
    /// 同时产生链接文本和结构化链接集合。
    TextAndLinks,
}

impl ActionOutputContract {
    /// 返回该契约允许的字段名；调用方以字段名集合而不是插入顺序解释结果。
    pub const fn keys(self) -> &'static [ActionOutputKey] {
        match self {
            Self::None => &[],
            Self::Text => &[ActionOutputKey::Text],
            Self::Value => &[ActionOutputKey::Value],
            Self::Item => &[ActionOutputKey::Item],
            Self::Items => &[ActionOutputKey::Items],
            Self::Links => &[ActionOutputKey::Links],
            Self::TextAndLinks => &[ActionOutputKey::Text, ActionOutputKey::Links],
        }
    }

    /// 校验后端输出没有悄悄改变动作的字段语义。
    pub fn validate(self, outputs: &BTreeMap<String, Value>) -> Result<(), OutputContractError> {
        let mut expected = self
            .keys()
            .iter()
            .map(|key| key.as_str().to_owned())
            .collect::<Vec<_>>();
        // ActionOutcome 使用 BTreeMap，跨后端比较必须采用相同的稳定字典序。
        expected.sort();
        let actual = outputs.keys().cloned().collect::<Vec<_>>();
        if actual == expected {
            return Ok(());
        }
        Err(OutputContractError { expected, actual })
    }
}

/// 后端实际输出与动作契约不一致时的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputContractError {
    /// 动作声明的字段集合。
    pub expected: Vec<String>,
    /// 后端实际返回的字段集合。
    pub actual: Vec<String>,
}

impl UiOperation {
    /// 返回 UI 节点声明的稳定输出契约。
    pub const fn output_contract(&self) -> ActionOutputContract {
        match self {
            Self::GetText { .. } => ActionOutputContract::Text,
            Self::GetValue { .. } => ActionOutputContract::Value,
            Self::Extract {
                cardinality: crate::ExtractCardinality::One,
                ..
            } => ActionOutputContract::Item,
            Self::Extract {
                cardinality: crate::ExtractCardinality::Many,
                ..
            } => ActionOutputContract::Items,
            Self::CollectLinks { .. } => ActionOutputContract::TextAndLinks,
            Self::Click { .. }
            | Self::SetValue { .. }
            | Self::PressKey { .. }
            | Self::TypeText { .. } => ActionOutputContract::None,
        }
    }
}

impl AutomationAction {
    /// 返回已解析动作声明的稳定输出契约。
    pub const fn output_contract(&self) -> ActionOutputContract {
        match self {
            Self::GetText { .. } => ActionOutputContract::Text,
            Self::GetValue { .. } => ActionOutputContract::Value,
            Self::Extract {
                cardinality: crate::ExtractCardinality::One,
                ..
            } => ActionOutputContract::Item,
            Self::Extract {
                cardinality: crate::ExtractCardinality::Many,
                ..
            } => ActionOutputContract::Items,
            Self::CollectLinks { .. } => ActionOutputContract::TextAndLinks,
            Self::Click { .. }
            | Self::SetValue { .. }
            | Self::PressKey { .. }
            | Self::TypeText { .. } => ActionOutputContract::None,
        }
    }
}
