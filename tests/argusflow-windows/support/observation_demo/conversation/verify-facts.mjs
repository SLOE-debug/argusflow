/** 对比原始事实与生成节点，不从执行计划构造答案。 */
export function verifyFacts(result, recording) {
  const scope = result.workflow?.scopes;
  if (!scope || scope.length !== 1)
    throw Error("本次录制要求一个显式线性作用域");
  const s = scope[0],
    byId = new Map(s.nodes.map((n) => [n.id, n]));
  const ordered = [];
  let current = { kind: "start" };
  for (let n = 0; n <= s.nodes.length; n++) {
    // 比较字段而非 JSON 顺序，模型可调整对象字段顺序。
    const match = s.edges.filter(
      (e) =>
        e.source.kind === current.kind &&
        (current.kind !== "node" || e.source.node === current.node),
    );
    if (match.length !== 1) throw Error("流程不是完整线性图");
    current = match[0].target;
    if (current.kind === "end") break;
    if (
      ordered.some((node) => node.id === current.node) ||
      !byId.has(current.node)
    )
      throw Error("节点引用无效");
    ordered.push(byId.get(current.node));
  }
  if (ordered.length !== s.nodes.length) throw Error("存在未执行的孤立节点");
  const annotations = new Map(
    result.analysis.node_evidence.map((e) => [e.node_id, e.evidence_ids]),
  );
  const canonical = (keys) =>
    keys
      .map((k) => k.replace(/^(Left|Right)(Control|Shift|Alt)$/, "$2"))
      .sort()
      .join("+");
  const literal = (expr) =>
    expr?.kind === "literal" ? expr.value?.value : undefined;
  let last = -1;
  let previousFact;
  const errors = [];
  for (const fact of recording.timeline.filter((e) =>
    ["keyboard_chord", "copy"].includes(e.kind),
  )) {
    const matches = ordered
      .map((node, index) => ({ node, index }))
      .filter(({ node }) => {
        if (!annotations.get(node.id)?.includes(fact.id)) return false;
        const task = node.action.task;
        if (task?.type_id === "demo.wechat_paste_send")
          return (
            fact.kind === "keyboard_chord" &&
            recording.targets[fact.target]?.title === "微信" &&
            ["Control+V", "Enter"].includes(canonical(fact.keys))
          );
        if (fact.kind === "keyboard_chord")
          return (
            task?.type_id === "aql.press_keys" &&
            canonical(task.config.keys.split("+")) === canonical(fact.keys)
          );
        return (
          task?.type_id === "browser.copy_text" &&
          literal(task.inputs.expected)?.slice(
            literal(task.inputs.start),
            literal(task.inputs.end),
          ) === fact.selection.text
        );
      });
    if (matches.length !== 1)
      errors.push(
        `${fact.id} 的 ${fact.kind === "copy" ? "复制 " + fact.selection.text : canonical(fact.keys)} 未对应唯一实际动作节点`,
      );
    else if (
      matches[0].index < last ||
      (matches[0].index === last &&
        !(
          matches[0].node.action.task.type_id === "demo.wechat_paste_send" &&
          canonical(previousFact?.keys ?? []) === "Control+V" &&
          canonical(fact.keys ?? []) === "Enter"
        ))
    )
      errors.push(`${fact.id} 动作顺序错误`);
    else last = matches[0].index;
    previousFact = fact;
  }
  const tasks = ordered.map((n) => n.action.task?.type_id);
  if (
    !tasks.includes("window.wait_document") ||
    !tasks.includes("file.wait_text")
  )
    errors.push(
      "每次记事本粘贴后必须有 window.wait_document（query 请求文本，expected 为累计正文），每次保存后必须有 file.wait_text。aql.wait 不代替 window.wait_document 的换行规范化校验。",
    );
  let active = "editor_window";
  for (const node of ordered) {
    const task = node.action.task;
    if (task?.type_id === "demo.wechat_paste_send") {
      active = "wechat_window";
      const references = annotations.get(node.id) ?? [];
      const events = recording.timeline.filter(
        (e) => references.includes(e.id) && e.kind === "keyboard_chord",
      );
      if (
        events.length !== 2 ||
        canonical(events[0].keys) !== "Control+V" ||
        canonical(events[1].keys) !== "Enter"
      )
        errors.push(`${node.id} 必须一一对应同轮微信粘贴与发送`);
      const samples = recording.source.samples.filter(
        (s) =>
          s.from_epoch_ms <= recording.origin + (events[0]?.ms ?? 0) &&
          s.clipboard_current,
      );
      if (literal(task.inputs.expected) !== samples.at(-1)?.clipboard_current)
        errors.push(`${node.id} 正文必须等于该轮粘贴前实际剪贴板`);
      if (
        literal(task.inputs.recipient) !== "文件传输助手" ||
        !recording.source.samples.some(
          (s) =>
            references.includes(s.id) &&
            s.ocr.some((b) => b.text === "文件传输助手"),
        )
      )
        errors.push(`${node.id} 缺少接收人证据`);
    }
    if (
      task?.type_id === "file.wait_text" &&
      (task.inputs.path?.kind !== "input" ||
        task.inputs.path.name !== "output_path")
    )
      errors.push(
        `${node.id} 的 path 必须引用运行输入 {input:"output_path"}，不能用字符串占位符`,
      );
    if (task?.type_id === "window.activate") active = task.resources.window;
    if (task?.type_id === "browser.copy_text") active = "browser_window";
    if (task?.type_id === "aql.press_keys" && active !== task.resources.scope)
      errors.push(
        `${node.id} 按键前缺少 window.activate 将 ${task.resources.scope} 置于前台`,
      );
  }
  if (errors.length) throw Error(errors.join("\n"));
  return {
    covered_events: recording.timeline.filter((e) =>
      ["keyboard_chord", "copy"].includes(e.kind),
    ).length,
    nodes: ordered.length,
  };
}
