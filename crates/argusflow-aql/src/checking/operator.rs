use crate::{MatchOperator, ValueType};

pub(crate) fn accepts(operator: MatchOperator, value: ValueType) -> bool {
    match operator {
        MatchOperator::Equal | MatchOperator::NotEqual => true,
        MatchOperator::Contains
        | MatchOperator::StartsWith
        | MatchOperator::EndsWith
        | MatchOperator::Regex => value == ValueType::Text,
        _ => value == ValueType::Number,
    }
}
