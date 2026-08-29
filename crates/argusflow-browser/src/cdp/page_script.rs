//! CDP 查询计划到封闭页面函数的序列化边界。

use argusflow_core::{
    AutomationAction, AutomationError, BackendKind, ElementRole, ExtractCardinality,
    FieldProjection, PropertyPredicate,
};
use serde::Serialize;

use super::{CdpCandidateSource, CdpPlanExpr};

/// 页面函数接受的稳定查询 DTO，只包含执行所需字段。
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PagePlan<'plan> {
    /// 角色和完整谓词集合。
    Match {
        /// 实际候选来源；页面解释器会拒绝尚未实现的来源。
        source: CdpCandidateSource,
        /// 目标语义角色。
        role: ElementRole,
        /// 页面内逐候选计算的完整谓词集合。
        predicates: Vec<&'plan PropertyPredicate>,
    },
    /// 后代关系。
    Descendant {
        /// 祖先计划。
        ancestor: Box<PagePlan<'plan>>,
        /// 后代目标计划。
        target: Box<PagePlan<'plan>>,
    },
    /// 直接子元素关系。
    Child {
        /// 父计划。
        parent: Box<PagePlan<'plan>>,
        /// 子目标计划。
        target: Box<PagePlan<'plan>>,
    },
    /// 当前 scope 结果补集。
    Not {
        /// 被排除的计划。
        query: Box<PagePlan<'plan>>,
    },
    /// 第一个结果。
    First {
        /// 内部计划。
        query: Box<PagePlan<'plan>>,
    },
    /// 一基索引结果。
    Nth {
        /// 内部计划。
        query: Box<PagePlan<'plan>>,
        /// 一基索引。
        index: usize,
    },
    /// 浏览器原生 CSS selector。
    Css {
        /// 完整 selector。
        selector: &'plan str,
    },
}

/// 页面函数接受的封闭动作 DTO，不暴露任意 JavaScript。
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PageAction<'action> {
    /// DOM click。
    Click,
    /// 原生 value setter 与 input/change 事件。
    SetValue {
        /// 已解析完整文本。
        value: &'action str,
    },
    /// 可见文本读取。
    GetText,
    /// value 属性读取。
    GetValue,
    /// 通用单目标或集合字段投影。
    Extract {
        /// 唯一目标或集合输出约束。
        cardinality: ExtractCardinality,
        /// 有序字段投影集合。
        fields: &'action [FieldProjection],
    },
    /// 链接标题与绝对 URL 批量投影。
    CollectLinks,
}

/// 把冻结查询和动作编码进固定解释器模板。
pub(super) fn build_page_action_script(
    expression: &CdpPlanExpr,
    action: &AutomationAction,
) -> Result<String, AutomationError> {
    let plan = PagePlan::from(expression);
    let action = match action {
        AutomationAction::Click { .. } => PageAction::Click,
        AutomationAction::SetValue { value, .. } => PageAction::SetValue { value },
        AutomationAction::GetText { .. } => PageAction::GetText,
        AutomationAction::GetValue { .. } => PageAction::GetValue,
        AutomationAction::Extract {
            cardinality,
            fields,
            ..
        } => PageAction::Extract {
            cardinality: *cardinality,
            fields,
        },
        AutomationAction::CollectLinks { .. } => PageAction::CollectLinks,
        AutomationAction::PressKey { .. } | AutomationAction::TypeText { .. } => {
            unreachable!("input actions are rejected by CdpBackend::prepare")
        }
    };
    let plan = serde_json::to_string(&plan).map_err(serialization_error)?;
    let action = serde_json::to_string(&action).map_err(serialization_error)?;
    Ok(PAGE_INTERPRETER
        .replace("__ARGUS_PLAN__", &plan)
        .replace("__ARGUS_ACTION__", &action))
}

impl<'plan> From<&'plan CdpPlanExpr> for PagePlan<'plan> {
    fn from(expression: &'plan CdpPlanExpr) -> Self {
        match expression {
            CdpPlanExpr::Match(matcher) => Self::Match {
                source: matcher.source,
                role: matcher.role,
                predicates: matcher.predicates.iter().collect(),
            },
            CdpPlanExpr::Descendant { ancestor, target } => Self::Descendant {
                ancestor: Box::new(Self::from(ancestor.as_ref())),
                target: Box::new(Self::from(target.as_ref())),
            },
            CdpPlanExpr::Child { parent, target } => Self::Child {
                parent: Box::new(Self::from(parent.as_ref())),
                target: Box::new(Self::from(target.as_ref())),
            },
            CdpPlanExpr::Not(query) => Self::Not {
                query: Box::new(Self::from(query.as_ref())),
            },
            CdpPlanExpr::First(query) => Self::First {
                query: Box::new(Self::from(query.as_ref())),
            },
            CdpPlanExpr::Nth { query, index } => Self::Nth {
                query: Box::new(Self::from(query.as_ref())),
                index: *index,
            },
            CdpPlanExpr::Css { selector } => Self::Css { selector },
        }
    }
}

/// 不应失败的 DTO JSON 编码错误映射。
fn serialization_error(error: serde_json::Error) -> AutomationError {
    AutomationError::BackendFailed {
        backend: BackendKind::BrowserCdp,
        message: error.to_string(),
    }
}

/// 固定页面解释器：累计扫描预算、关系 scope、谓词和动作均为封闭集合。
const PAGE_INTERPRETER: &str = r#"(() => {
  const plan = __ARGUS_PLAN__;
  const action = __ARGUS_ACTION__;
  const maxVisitedNodes = 10000;
  let visitedNodes = 0;

  const unique = (elements) => Array.from(new Set(elements));
  const observe = (elements) => {
    visitedNodes += elements.length;
    if (visitedNodes > maxVisitedNodes) {
      throw new Error(`CDP traversal exceeded ${maxVisitedNodes} DOM nodes`);
    }
    return elements;
  };
  const scopedElements = (roots, direct) => unique(roots.flatMap((root) => {
    if (root === document) {
      return direct ? [document.documentElement] : Array.from(document.querySelectorAll('*'));
    }
    return direct ? Array.from(root.children) : Array.from(root.querySelectorAll('*'));
  }));
  const implicitRole = (element) => {
    const tag = element.tagName.toLowerCase();
    const inputType = tag === 'input' ? (element.type || 'text').toLowerCase() : '';
    if (tag === 'a' && element.hasAttribute('href')) return 'link';
    if (tag === 'button' || (tag === 'input' && ['button', 'submit', 'reset'].includes(inputType))) return 'button';
    if (tag === 'textarea' || (tag === 'input' && !['button', 'submit', 'reset', 'checkbox', 'radio'].includes(inputType))) return 'text_box';
    if (tag === 'input' && inputType === 'checkbox') return 'check_box';
    if (tag === 'input' && inputType === 'radio') return 'radio';
    if (tag === 'select') return 'combo_box';
    if (tag === 'ul' || tag === 'ol') return 'list';
    if (tag === 'li') return 'list_item';
    if (tag === 'table') return 'table';
    if (tag === 'tr') return 'row';
    if (tag === 'td' || tag === 'th') return 'cell';
    if (tag === 'img') return 'image';
    if (tag === 'nav') return 'menu';
    if (tag === 'body' || tag === 'html') return 'document';
    if (element.children.length === 0) return 'text';
    return 'pane';
  };
  const roleMatches = (element, role) => {
    const explicit = (element.getAttribute('role') || '').toLowerCase().replaceAll('-', '_');
    const normalized = explicit || implicitRole(element);
    const aliases = {
      textbox: 'text_box', checkbox: 'check_box', listitem: 'list_item',
      treeitem: 'tree_item', tablist: 'tab', menuitem: 'menu_item',
      img: 'image', grid: 'table', gridcell: 'cell',
    };
    return (aliases[normalized] || normalized) === role;
  };
  const accessibleName = (element) => {
    const labelledBy = (element.getAttribute('aria-labelledby') || '')
      .split(/\s+/).filter(Boolean)
      .map((id) => document.getElementById(id)?.textContent || '').join(' ').trim();
    if (labelledBy) return labelledBy;
    if (element.getAttribute('aria-label')) return element.getAttribute('aria-label').trim();
    if (element.labels?.length) return Array.from(element.labels).map((label) => label.textContent || '').join(' ').trim();
    return (element.getAttribute('alt') || element.getAttribute('title') || element.innerText || element.textContent || '').replace(/\s+/g, ' ').trim();
  };
  const attributeValue = (element, attribute) => {
    switch (attribute.type) {
      case 'name': return accessibleName(element);
      case 'key': return element.id || element.getAttribute('data-key') || '';
      case 'value': return typeof element.value === 'string' ? element.value : (element.getAttribute('value') || '');
      case 'enabled': return !element.disabled && element.getAttribute('aria-disabled') !== 'true';
      case 'visible': {
        const style = getComputedStyle(element);
        return style.display !== 'none' && style.visibility !== 'hidden' && element.getClientRects().length > 0;
      }
      case 'focused': return document.activeElement === element;
      case 'checked': return Boolean(element.checked) || element.getAttribute('aria-checked') === 'true';
      case 'selected': return Boolean(element.selected) || element.getAttribute('aria-selected') === 'true';
      case 'dom':
        return attribute.attribute === 'test_id'
          ? (element.getAttribute('data-testid') || '')
          : (typeof element.className === 'string' ? element.className : '');
      default: return '';
    }
  };
  const predicateMatches = (element, predicate) => {
    const left = attributeValue(element, predicate.attribute);
    const right = predicate.value.value;
    switch (predicate.operator) {
      case 'equal': return left === right;
      case 'not_equal': return left !== right;
      case 'contains': return typeof left === 'string' && left.includes(right);
      case 'starts_with': return typeof left === 'string' && left.startsWith(right);
      case 'ends_with': return typeof left === 'string' && left.endsWith(right);
      case 'regex': return new RegExp(right.pattern, right.case_insensitive ? 'iu' : 'u').test(String(left));
      default: return false;
    }
  };
  const evaluate = (node, roots = [document], direct = false) => {
    switch (node.type) {
      case 'match':
        if (node.source !== 'dom') {
          throw new Error(`unsupported CDP candidate source ${node.source}`);
        }
        return observe(scopedElements(roots, direct)).filter((element) => (
          roleMatches(element, node.role)
          && node.predicates.every((predicate) => predicateMatches(element, predicate))
        ));
      case 'css':
        return observe(unique(roots.flatMap((root) => {
          if (direct) return scopedElements([root], true).filter((element) => element.matches(node.selector));
          return Array.from(root.querySelectorAll(node.selector));
        })));
      case 'descendant':
        return unique(evaluate(node.ancestor, roots, direct).flatMap((ancestor) => evaluate(node.target, [ancestor], false)));
      case 'child':
        return unique(evaluate(node.parent, roots, direct).flatMap((parent) => evaluate(node.target, [parent], true)));
      case 'not': {
        const excluded = new Set(evaluate(node.query, roots, direct));
        return observe(scopedElements(roots, direct)).filter((element) => !excluded.has(element));
      }
      case 'first': return evaluate(node.query, roots, direct).slice(0, 1);
      case 'nth': return evaluate(node.query, roots, direct).slice(node.index - 1, node.index);
      default: throw new Error(`unsupported CDP plan node ${node.type}`);
    }
  };

  const elements = evaluate(plan);
  if (elements.length === 0) return { status: 'not_found', matches: 0 };
  if (action.type === 'extract') {
    const projectField = (element, field) => {
      switch (field.source.type) {
        case 'text': return (element.innerText || element.textContent || '').replace(/\s+/g, ' ').trim();
        case 'value': return typeof element.value === 'string' ? element.value : (element.getAttribute('value') || '');
        case 'name': return accessibleName(element);
        case 'property': {
          const value = element[field.source.name];
          return ['string', 'number', 'boolean'].includes(typeof value) ? value : null;
        }
        case 'attribute': {
          if (field.source.name === 'href' && typeof element.href === 'string') return element.href;
          if (field.source.name === 'src' && typeof element.src === 'string') return element.src;
          return element.getAttribute(field.source.name);
        }
        default: throw new Error(`unsupported extract source ${field.source.type}`);
      }
    };
    const project = (element) => Object.fromEntries(action.fields.map((field) => (
      [field.name, projectField(element, field)]
    )));
    if (action.cardinality === 'one') {
      if (elements.length !== 1) return { status: 'ambiguous', matches: elements.length };
      return { status: 'ok', message: '已通过 CDP 提取 1 个目标', outputs: { item: project(elements[0]) } };
    }
    const items = elements.map(project);
    return { status: 'ok', message: `已通过 CDP 批量提取 ${items.length} 个目标`, outputs: { items } };
  }
  if (action.type === 'collect_links') {
    const links = elements.map((element) => {
      const anchor = element instanceof HTMLAnchorElement
        ? element
        : (element.closest('a[href]') || element.querySelector('a[href]'));
      if (!anchor?.href) return null;
      const title = (element.innerText || element.textContent || '').replace(/\s+/g, ' ').trim();
      return title ? { title, url: anchor.href } : null;
    }).filter(Boolean);
    if (links.length === 0) return { status: 'not_found', matches: 0 };
    const text = links.map((link) => `${link.title}\t${link.url}`).join('\r\n') + '\r\n';
    return { status: 'ok', message: `已通过 CDP 批量读取 ${links.length} 条链接`, outputs: { links, text } };
  }
  if (elements.length !== 1) return { status: 'ambiguous', matches: elements.length };
  const element = elements[0];
  if (action.type === 'click') {
    element.click();
    return { status: 'ok', message: '已通过 CDP 调用 DOM click', outputs: {} };
  }
  if (action.type === 'set_value') {
    const prototype = element instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    const descriptor = Object.getOwnPropertyDescriptor(prototype, 'value');
    if (!descriptor?.set) throw new Error('target does not expose a writable value property');
    descriptor.set.call(element, action.value);
    element.dispatchEvent(new Event('input', { bubbles: true }));
    element.dispatchEvent(new Event('change', { bubbles: true }));
    return { status: 'ok', message: '已通过 CDP 原生 value setter 写入目标', outputs: {} };
  }
  if (action.type === 'get_text') {
    const text = (element.innerText || element.textContent || '').trim();
    return { status: 'ok', message: '已通过 CDP 读取目标文本', outputs: { text } };
  }
  const value = typeof element.value === 'string' ? element.value : (element.getAttribute('value') || '');
  return { status: 'ok', message: '已通过 CDP 读取目标值', outputs: { value } };
})()"#;

#[cfg(test)]
mod tests {
    use argusflow_core::{
        AqlQuery, AutomationAction, AutomationTarget, ExtractCardinality, FieldProjection,
        FieldProjectionSource,
    };

    use super::build_page_action_script;
    use crate::cdp::CdpPlanExpr;

    #[test]
    fn collect_links_script_uses_crlf_and_structured_outputs() {
        let action = AutomationAction::CollectLinks {
            target: AutomationTarget::query(AqlQuery::v1("css(\"a.news\")")),
        };

        let script = build_page_action_script(
            &CdpPlanExpr::Css {
                selector: "a.news".to_owned(),
            },
            &action,
        )
        .expect("script should compile");

        assert!(script.contains("join('\\r\\n') + '\\r\\n'"));
        assert!(script.contains("element.closest('a[href]')"));
        assert!(script.contains("outputs: { links, text }"));
        assert!(script.contains(r#""type":"css""#));
        assert!(!script.contains("__ARGUS_PLAN__"));
    }

    #[test]
    fn extract_many_serializes_generic_field_projections() {
        let action = AutomationAction::Extract {
            target: AutomationTarget::query(AqlQuery::v1("css(\"a.news\")")),
            cardinality: ExtractCardinality::Many,
            fields: vec![
                FieldProjection {
                    name: "title".to_owned(),
                    source: FieldProjectionSource::Text,
                },
                FieldProjection {
                    name: "url".to_owned(),
                    source: FieldProjectionSource::Attribute {
                        name: "href".to_owned(),
                    },
                },
            ],
        };

        let script = build_page_action_script(
            &CdpPlanExpr::Css {
                selector: "a.news".to_owned(),
            },
            &action,
        )
        .expect("extract script should compile");

        assert!(script.contains(r#""type":"extract""#));
        assert!(script.contains(r#""cardinality":"many""#));
        assert!(script.contains(r#""name":"href""#));
        assert!(script.contains("outputs: { items }"));
    }

    #[test]
    fn semantic_matcher_preserves_candidate_source_in_page_dto() {
        use argusflow_core::ElementRole;

        use crate::cdp::{CdpCandidateSource, CdpMatcherPlan};

        let action = AutomationAction::GetText {
            target: AutomationTarget::query(AqlQuery::v1("button")),
        };
        let expression = CdpPlanExpr::Match(CdpMatcherPlan {
            source: CdpCandidateSource::Dom,
            role: ElementRole::Button,
            predicates: Vec::new(),
        });

        let script = build_page_action_script(&expression, &action)
            .expect("semantic matcher should serialize");

        assert!(script.contains(r#""source":"dom""#));
        assert!(script.contains("unsupported CDP candidate source"));
    }
}
