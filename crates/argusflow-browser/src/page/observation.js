(() => {
  if (globalThis.__argusflowRecorderV1) return;
  const events = [];
  const identities = new WeakMap();
  let nextNode = 0,
    sequence = 0,
    lost = 0,
    characters = 0;
  const types = [
    "click",
    "contextmenu",
    "input",
    "change",
    "compositionend",
    "submit",
    "pointerdown",
    "pointerup",
    "pointercancel",
    "selectionchange",
    "copy",
    "cut",
    "paste",
  ];
  /** 节点身份只在该文档实例内稳定，不以相同文字认定同一节点。 */
  const identity = (node) => {
    if (!identities.has(node)) identities.set(node, ++nextNode);
    return identities.get(node);
  };
  const activeElement = () => {
    let node = document.activeElement;
    for (let depth = 0; node?.shadowRoot?.activeElement && depth < 16; depth++)
      node = node.shadowRoot.activeElement;
    return node;
  };
  const password = (node) =>
    node instanceof HTMLInputElement && node.type === "password";
  /** 不把 UTF-16 代理对切开，保留明确的截断状态。 */
  const prefix = (value, limit) => {
    let end = Math.min(value.length, limit);
    if (end < value.length && /[\uD800-\uDBFF]/.test(value[end - 1] || ""))
      end--;
    return value.slice(0, end);
  };
  const describe = (node) => {
    if (!(node instanceof Element)) return null;
    const sensitive = password(node);
    const field =
      node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement;
    const value = sensitive
      ? null
      : field || node instanceof HTMLSelectElement
        ? node.value
        : null;
    const rect = node.getBoundingClientRect();
    const context = [];
    let parent = node.parentElement;
    for (let i = 0; parent && i < 4; i++, parent = parent.parentElement)
      context.push(parent.tagName + "#" + prefix(parent.id, 128));
    if (node.getRootNode() instanceof ShadowRoot)
      context.push("open-shadow-root");
    const name = sensitive
      ? "[密码字段]"
      : node.getAttribute("aria-label") ||
        node.getAttribute("title") ||
        node.textContent ||
        "";
    return {
      node: identity(node),
      editable:
        node.isContentEditable || (field && !node.readOnly && !node.disabled),
      tag: node.tagName,
      role: node.getAttribute("role") || "",
      name: prefix(name, 512),
      id: prefix(node.id, 128),
      input_type: node.getAttribute("type") || "",
      value: value === null ? null : prefix(value, 8192),
      password: sensitive,
      bounds: [rect.left, rect.top, rect.right, rect.bottom],
      context,
      truncated:
        !!parent ||
        name.length > 512 ||
        (value?.length ?? 0) > 8192 ||
        node.tagName === "IFRAME",
    };
  };
  const endpoint = (node, offset) =>
    node
      ? {
          node: identity(node),
          node_type: node.nodeType,
          offset,
          text: prefix(node.textContent || "", 512),
          parent: describe(node.parentElement),
        }
      : null;
  /** 同时覆盖表单字段和普通 DOM；不调用 select/focus 或剪贴板 API。 */
  const selection = () => {
    try {
      const active = activeElement();
      if (password(active)) return { kind: "sensitive" };
      if (
        active instanceof HTMLInputElement ||
        active instanceof HTMLTextAreaElement
      ) {
        const start = active.selectionStart,
          end = active.selectionEnd;
        if (start === null || end === null) return { kind: "unsupported" };
        return {
          kind: "control",
          node: identity(active),
          start,
          end,
          direction: active.selectionDirection || "none",
          text: prefix(active.value.slice(start, end), 8192),
          truncated: end - start > 8192,
        };
      }
      const selected = window.getSelection();
      if (!selected) return { kind: "unsupported" };
      const text = selected.toString();
      const rects = [];
      let truncated = text.length > 8192;
      for (let index = 0; index < selected.rangeCount; index++) {
        for (const rect of selected.getRangeAt(index).getClientRects()) {
          if (rects.length >= 32) {
            truncated = true;
            break;
          }
          rects.push([rect.left, rect.top, rect.right, rect.bottom]);
        }
        if (rects.length >= 32) break;
      }
      return {
        kind: "document",
        anchor: endpoint(selected.anchorNode, selected.anchorOffset),
        focus: endpoint(selected.focusNode, selected.focusOffset),
        collapsed: selected.isCollapsed,
        range_count: selected.rangeCount,
        text: prefix(text, 8192),
        rects,
        truncated,
      };
    } catch {
      return { kind: "unavailable" };
    }
  };
  const receive = (event) => {
    if (events.length >= 128) {
      lost++;
      return;
    }
    const node =
      event.type === "selectionchange"
        ? activeElement()
        : event.composedPath()[0];
    const pointer = event instanceof MouseEvent;
    const item = {
      sequence: ++sequence,
      kind: event.type,
      time: performance.now(),
      trusted: event.isTrusted,
      input_type: event.inputType || "",
      target: describe(node),
      selection: password(node) ? { kind: "sensitive" } : selection(),
      point: pointer ? [event.clientX, event.clientY] : null,
      button: pointer ? event.button : null,
    };
    const length = JSON.stringify(item).length;
    if (characters + length > 128 * 1024) {
      lost++;
      return;
    }
    characters += length;
    events.push(item);
  };
  types.forEach((type) =>
    document.addEventListener(type, receive, { capture: true, passive: true }),
  );
  Object.defineProperty(globalThis, "__argusflowRecorderV1", {
    configurable: true,
    value: {
      take() {
        const result = {
          page_now: performance.now(),
          focused: document.hasFocus(),
          lost,
          events: events.splice(0),
          active: describe(activeElement()),
          selection: selection(),
        };
        lost = 0;
        characters = 0;
        return result;
      },
      stop() {
        types.forEach((type) =>
          document.removeEventListener(type, receive, true),
        );
        events.length = 0;
        delete globalThis.__argusflowRecorderV1;
      },
    },
  });
})();
