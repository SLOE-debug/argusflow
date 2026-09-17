(() => {
  if (globalThis.__argusflowRecorderV1) return;
  const events = [];
  let sequence = 0, lost = 0, characters = 0;
  const types = ["click", "contextmenu", "input", "change", "compositionend", "submit"];
  const describe = (node) => {
    if (!(node instanceof Element)) return null;
    const password = node instanceof HTMLInputElement && node.type === "password";
    const value = password ? null : ((node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement || node instanceof HTMLSelectElement) ? node.value : null);
    const rect = node.getBoundingClientRect();
    const context = [];
    let parent = node.parentElement;
    for (let i = 0; parent && i < 4; i++, parent = parent.parentElement) context.push(parent.tagName + "#" + parent.id.slice(0, 128));
    const shadow = node.getRootNode() instanceof ShadowRoot;
    if (shadow) context.push("open-shadow-root");
    const name = password ? "[密码字段]" : (node.getAttribute("aria-label") || node.getAttribute("title") || node.textContent || "");
    return { tag: node.tagName, role: node.getAttribute("role") || "", name: name.slice(0, 512), id: node.id.slice(0, 128), input_type: node.getAttribute("type") || "", value: value?.slice(0, 8192) ?? null, password, bounds: [rect.left, rect.top, rect.right, rect.bottom], context, truncated: !!parent || name.length > 512 || (value?.length ?? 0) > 8192 || node.tagName === "IFRAME" };
  };
  const receive = (event) => {
    if (events.length >= 128) { lost++; return; }
    const item = { sequence: ++sequence, kind: event.type, time: performance.now(), trusted: event.isTrusted, input_type: event.inputType || "", target: describe(event.composedPath()[0]) };
    const length = JSON.stringify(item).length;
    if (characters + length > 128 * 1024) { lost++; return; }
    characters += length;
    events.push(item);
  };
  types.forEach(type => document.addEventListener(type, receive, { capture: true, passive: true }));
  Object.defineProperty(globalThis, "__argusflowRecorderV1", { configurable: true, value: {
    take() { const result = { page_now: performance.now(), focused: document.hasFocus(), lost, events: events.splice(0), active: describe(document.activeElement) }; lost = 0; characters = 0; return result; },
    stop() { types.forEach(type => document.removeEventListener(type, receive, true)); events.length = 0; delete globalThis.__argusflowRecorderV1; }
  }});
})();
