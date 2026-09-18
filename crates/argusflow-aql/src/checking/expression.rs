use crate::{AqlError, DiagnosticCode, Expr};

#[derive(PartialEq)]
enum Output {
    Elements,
    Scope,
}

pub(crate) fn validate(expression: &Expr) -> Result<(), AqlError> {
    require_elements(output(expression)?)
}

fn require_elements(output: Output) -> Result<(), AqlError> {
    if output == Output::Scope {
        Err(AqlError::general(
            DiagnosticCode::Type,
            "文档边界需要使用 > 或 >> 指定其中的元素",
        ))
    } else {
        Ok(())
    }
}

fn output(expression: &Expr) -> Result<Output, AqlError> {
    match expression {
        Expr::Match { .. } | Expr::Css(_) => Ok(Output::Elements),
        Expr::Nth { query, .. } | Expr::Position { query, .. } => {
            require_elements(output(query)?)?;
            Ok(Output::Elements)
        }
        Expr::Enter { host, .. } => {
            require_elements(output(host)?)?;
            Ok(Output::Scope)
        }
        Expr::Relation { left, right, .. } => {
            output(left)?;
            output(right)
        }
        Expr::Spatial(spatial) => {
            require_elements(output(&spatial.anchor)?)?;
            require_elements(output(&spatial.target)?)?;
            for inner in [&spatial.region, &spatial.second_anchor]
                .into_iter()
                .flatten()
            {
                require_elements(output(inner)?)?;
            }
            Ok(Output::Elements)
        }
    }
}
