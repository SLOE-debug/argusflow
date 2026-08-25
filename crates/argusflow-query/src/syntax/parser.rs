use crate::{
    Diagnostic, DiagnosticCode, DiagnosticParams, DiagnosticSeverity, EditorPosition, EditorRange,
};

use super::{CstElement, CstNode, CstNodeKind, RawToken, RawTokenKind, SyntaxTree};

/// 用平衡括号恢复策略构造始终存在的 CST，并收集多个结构诊断。
pub(crate) fn build_recovery_tree(
    tokens: Vec<RawToken>,
    mut diagnostics: Vec<Diagnostic>,
) -> (SyntaxTree, Vec<Diagnostic>) {
    let document_end = tokens
        .last()
        .map_or(EditorPosition::default(), |token| token.range.end);
    let mut stack = vec![NodeBuilder::document(document_end)];

    for token in tokens.iter().cloned() {
        match token.kind {
            RawTokenKind::LeftParen => {
                let mut call = NodeBuilder::call(token.range.start);
                call.push_token(token);
                stack.push(call);
            }
            RawTokenKind::RightParen if stack.len() > 1 => {
                let mut completed = stack.pop().expect("call builder exists");
                completed.push_token(token);
                let node = completed.finish(CstNodeKind::Call);
                stack
                    .last_mut()
                    .expect("parent builder exists")
                    .push_node(node);
            }
            RawTokenKind::RightParen => {
                diagnostics.push(Diagnostic {
                    code: DiagnosticCode::UnexpectedRightParenthesis,
                    severity: DiagnosticSeverity::Error,
                    range: Some(token.range),
                    backend: None,
                    params: DiagnosticParams::None,
                });
                stack
                    .last_mut()
                    .expect("document builder exists")
                    .push_token(token);
            }
            RawTokenKind::Error => {
                stack.last_mut().expect("builder exists").contains_error = true;
                stack.last_mut().expect("builder exists").push_token(token);
            }
            _ => stack.last_mut().expect("builder exists").push_token(token),
        }
    }

    while stack.len() > 1 {
        let unfinished = stack.pop().expect("unfinished call exists");
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::MissingRightParenthesis,
            severity: DiagnosticSeverity::Error,
            range: Some(EditorRange {
                start: unfinished.start,
                end: document_end,
            }),
            backend: None,
            params: DiagnosticParams::Expected {
                expected: ")".to_owned(),
            },
        });
        let node = unfinished.finish(CstNodeKind::Error);
        stack
            .last_mut()
            .expect("parent builder exists")
            .push_node(node);
    }

    let root = stack
        .pop()
        .expect("document builder exists")
        .finish(CstNodeKind::Document);
    (SyntaxTree { root, tokens }, diagnostics)
}

/// Recovery CST 节点的增量构造状态。
struct NodeBuilder {
    /// 节点起点。
    start: EditorPosition,
    /// 最近一个子元素的终点。
    end: EditorPosition,
    /// 是否包含错误 token。
    contains_error: bool,
    /// 已恢复的子元素。
    children: Vec<CstElement>,
}

impl NodeBuilder {
    /// 创建文档根构造器。
    fn document(end: EditorPosition) -> Self {
        Self {
            start: EditorPosition::default(),
            end,
            contains_error: false,
            children: Vec::new(),
        }
    }

    /// 创建左括号后的调用构造器。
    fn call(start: EditorPosition) -> Self {
        Self {
            start,
            end: start,
            contains_error: false,
            children: Vec::new(),
        }
    }

    /// 追加 lossless token。
    fn push_token(&mut self, token: RawToken) {
        self.end = token.range.end;
        self.children.push(CstElement::Token { token });
    }

    /// 追加完成或恢复的子节点。
    fn push_node(&mut self, node: CstNode) {
        self.end = node.range.end;
        self.children.push(CstElement::Node { node });
    }

    /// 完成不可变 CST 节点。
    fn finish(self, requested_kind: CstNodeKind) -> CstNode {
        let kind = if self.contains_error {
            CstNodeKind::Error
        } else {
            requested_kind
        };
        CstNode {
            kind,
            range: EditorRange {
                start: self.start,
                end: self.end,
            },
            children: self.children,
        }
    }
}
