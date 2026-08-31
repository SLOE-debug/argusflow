//! CDP 只读观测页面函数；与动作执行脚本独立演进。

use argusflow_core::{AutomationError, BackendKind};

use super::{CdpPlanExpr, page_plan::PagePlan};

/// 把每个 selector 的有序备选计划编码进单次页面观察解释器。
pub(super) fn build_page_observation_script(
    expressions: &[Vec<CdpPlanExpr>],
) -> Result<String, AutomationError> {
    let plans = expressions
        .iter()
        .map(|alternatives| alternatives.iter().map(PagePlan::from).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let plans = serde_json::to_string(&plans).map_err(serialization_error)?;
    Ok(PAGE_OBSERVATION_INTERPRETER.replace("__ARGUS_PLANS__", &plans))
}

/// 不应失败的 DTO JSON 编码错误映射。
fn serialization_error(error: serde_json::Error) -> AutomationError {
    AutomationError::BackendFailed {
        backend: BackendKind::BrowserCdp,
        message: error.to_string(),
    }
}

/// 固定只读页面观察器；全部 selector 在同一次 `Runtime.evaluate` 中读取页面状态。
const PAGE_OBSERVATION_INTERPRETER: &str = r#"(() => {
  const plans = __ARGUS_PLANS__;
  const unique = (elements) => Array.from(new Set(elements));
  const scopedElements = (roots, direct) => unique(roots.flatMap((root) => {
    if (root === document) return direct ? [document.documentElement] : Array.from(document.querySelectorAll('*'));
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
    if (tag === 'body' || tag === 'html') return 'document';
    if (element.children.length === 0) return 'text';
    return 'pane';
  };
  const normalizedRole = (element) => {
    const explicit = (element.getAttribute('role') || '').toLowerCase().replaceAll('-', '_');
    const role = explicit || implicitRole(element);
    const aliases = { textbox: 'text_box', checkbox: 'check_box', listitem: 'list_item', treeitem: 'tree_item', tablist: 'tab', menuitem: 'menu_item', img: 'image', grid: 'table', gridcell: 'cell' };
    return aliases[role] || role;
  };
  const accessibleName = (element) => {
    const labelledBy = (element.getAttribute('aria-labelledby') || '').split(/\s+/).filter(Boolean)
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
      case 'visible': { const style = getComputedStyle(element); return style.display !== 'none' && style.visibility !== 'hidden' && element.getClientRects().length > 0; }
      case 'focused': return document.activeElement === element;
      case 'checked': return Boolean(element.checked) || element.getAttribute('aria-checked') === 'true';
      case 'selected': return Boolean(element.selected) || element.getAttribute('aria-selected') === 'true';
      case 'dom': return attribute.attribute === 'test_id' ? (element.getAttribute('data-testid') || '') : (typeof element.className === 'string' ? element.className : '');
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
      case 'match': return scopedElements(roots, direct).filter((element) => normalizedRole(element) === node.role && node.predicates.every((predicate) => predicateMatches(element, predicate)));
      case 'css': return unique(roots.flatMap((root) => direct ? scopedElements([root], true).filter((element) => element.matches(node.selector)) : Array.from(root.querySelectorAll(node.selector))));
      case 'descendant': return unique(evaluate(node.ancestor, roots, direct).flatMap((ancestor) => evaluate(node.target, [ancestor], false)));
      case 'child': return unique(evaluate(node.parent, roots, direct).flatMap((parent) => evaluate(node.target, [parent], true)));
      case 'not': { const excluded = new Set(evaluate(node.query, roots, direct)); return scopedElements(roots, direct).filter((element) => !excluded.has(element)); }
      case 'first': return evaluate(node.query, roots, direct).slice(0, 1);
      case 'nth': return evaluate(node.query, roots, direct).slice(node.index - 1, node.index);
      default: throw new Error(`unsupported CDP observation node ${node.type}`);
    }
  };
  const snapshot = (element) => {
    const rect = element.getBoundingClientRect();
    const text = (element.innerText || element.textContent || '').replace(/\s+/g, ' ').trim();
    const value = typeof element.value === 'string' ? element.value : (element.getAttribute('value') || '');
    return {
      name: accessibleName(element) || null,
      text: text || null,
      value: value || null,
      role: normalizedRole(element),
      bounds: { space: 'browser_viewport_css', x: rect.x, y: rect.y, width: rect.width, height: rect.height },
      confidence: null,
      source: 'browser_cdp',
    };
  };
  return plans.map((alternatives) => {
    const entities = alternatives
      .map((plan) => evaluate(plan))
      .find((matches) => matches.length > 0) || [];
    return { entities: entities.map(snapshot), complete: true };
  });
})()"#;
