/** 只展开正式节点样板与类型；不增加、删除、重排 AI 提交的任务。 */
export function lowerPlan(plan) {
  if (
    !Array.isArray(plan.nodes) ||
    plan.nodes.length < 1 ||
    plan.nodes.length > 200
  )
    throw Error("任务数量必须为1–200");
  const argument = (value) => {
    if (value && typeof value === "object") {
      if (Object.keys(value).length === 1 && typeof value.input === "string")
        return { kind: "input", name: value.input };
      if (
        Object.keys(value).length === 2 &&
        typeof value.node === "string" &&
        typeof value.output === "string"
      )
        return { kind: "node_output", node: value.node, output: value.output };
      throw Error("参数对象只能是 {input} 或 {node,output} 引用");
    }
    const type =
      typeof value === "string"
        ? "text"
        : typeof value === "boolean"
          ? "bool"
          : Number.isSafeInteger(value)
            ? "int"
            : null;
    if (!type) throw Error("此编译入口接受 text/bool/int 常量");
    return { kind: "literal", value_type: { type }, value: { type, value } };
  };
  const ids = new Set();
  const nodes = plan.nodes.map((node) => {
    if (typeof node.id !== "string" || ids.has(node.id))
      throw Error("节点ID必须唯一");
    ids.add(node.id);
    if (Object.values(node.resources ?? {}).some((v) => typeof v !== "string"))
      throw Error("资源绑定值必须是资源名字符串");
    return {
      id: node.id,
      timeout_ms: argument(
        node.type_id === "demo.wechat_paste_send" ? 60000 : 10000,
      ),
      output_bindings: {},
      action: {
        kind: "task",
        task: {
          type_id: node.type_id,
          version: 1,
          config: node.config ?? {},
          inputs: Object.fromEntries(
            Object.entries(node.inputs ?? {}).map(([k, v]) => [k, argument(v)]),
          ),
          resources: node.resources ?? {},
          resource_outputs: {},
          retry: null,
        },
      },
    };
  });
  let source = { kind: "start" };
  const edges = nodes.map((node, index) => {
    const target = { kind: "node", node: node.id },
      edge = { id: `edge-${index}`, source, target };
    source = target;
    return edge;
  });
  edges.push({ id: `edge-${nodes.length}`, source, target: { kind: "end" } });
  return {
    workflow: {
      name: "录制事实推导的文本传输",
      inputs: { output_path: { type: "text" } },
      outputs: {},
      resources: {
        browser_page: "automation.page",
        browser_window: "automation.window",
        editor_window: "automation.window",
      },
      root: "root",
      scopes: [{ id: "root", nodes, edges, outputs: {} }],
      subflows: {},
    },
    analysis: {
      summary: plan.summary,
      node_evidence: plan.nodes.map((n) => ({
        node_id: n.id,
        evidence_ids: n.evidence_ids ?? [],
        outcome: "uncertain",
        uncertainty: "待真实回放校验",
      })),
      unresolved: [],
      required_bindings: [],
      replay_ready: false,
    },
  };
}
