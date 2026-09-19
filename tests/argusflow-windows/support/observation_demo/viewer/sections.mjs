import { el, pill, button, jsonBlock, sourceButtons } from "./dom.mjs";

export function renderSources(data) {
  const box = el("div", "space-y-4");
  box.append(
    el("h2", "text-2xl font-semibold", "现有录制器能采到什么"),
    el(
      "p",
      "max-w-4xl text-sm leading-7 text-stone-600",
      "下表按当前代码接入情况核对。“已接入代码”不代表本次从正式录制界面导出了这些文件；本次实际运行的是 observation_demo。",
    ),
  );
  for (const item of data.capabilities) {
    const card = el(
      "section",
      "rounded-2xl border border-stone-200 bg-white p-6",
    );
    const header = el(
      "div",
      "flex flex-wrap items-center justify-between gap-3",
    );
    header.append(
      el("h3", "text-lg font-semibold", item.name),
      pill(item.status),
    );
    card.append(
      header,
      el("p", "mt-3 text-sm leading-7", item.fact),
      el("p", "mt-2 text-sm leading-7 text-amber-800", item.boundary),
      sourceButtons(item.sources, data),
    );
    box.append(card);
  }
  return box;
}

export function renderPresets(data) {
  const box = el("div", "space-y-4");
  box.append(
    el("h2", "text-2xl font-semibold", "哪些内容是程序预设的"),
    el(
      "p",
      "text-sm leading-7 text-stone-600",
      "这一页是执行计划审计，不属于录制证据，也没有作为模型输入。",
    ),
  );
  for (const item of data.presets) {
    const card = el(
      "section",
      "rounded-2xl border border-amber-200 bg-amber-50 p-6",
    );
    card.append(
      el("h3", "font-semibold", item.title),
      el("p", "mt-3 text-sm leading-7", item.text),
      sourceButtons(item.sources, data),
    );
    box.append(card);
  }
  const table = el(
    "div",
    "overflow-hidden rounded-xl border border-stone-200 bg-white",
  );
  const labels = {
    browser_copy: "计划：复制浏览器条目",
    document_paste: "计划：粘贴到记事本",
    document_copy: "计划：复制记事本某行",
  };
  data.plan.steps.forEach((step, index) => {
    const row = el(
      "div",
      "grid gap-2 border-b border-stone-100 p-4 md:grid-cols-3",
    );
    row.append(
      el("div", "font-mono text-xs text-stone-500", `预设 ${index + 1}`),
      el("div", "text-sm", labels[step.action] ?? step.action),
      el(
        "div",
        "break-all font-mono text-xs text-stone-500",
        step.selector
          ? `${step.selector} · index=${step.index}`
          : step.line !== undefined
            ? `line=${step.line}（脚本从 0 计数）`
            : "真实剪贴板粘贴",
      ),
    );
    table.append(row);
  });
  box.append(table, jsonBlock(data.plan, "查看完整 actor-private.json"));
  return box;
}

export function renderModels(data, jump) {
  const box = el("div", "space-y-5");
  box.append(
    el("h2", "text-2xl font-semibold", "AI 如何解释这些证据"),
    el(
      "p",
      "text-sm leading-7 text-stone-600",
      "下面是模型保存的原始草稿。点击证据编号可跳回时间线。文字、行号或定位仍可能错误；此页面不会执行 workflow，也不发送新的模型请求。",
    ),
  );
  const select = el(
    "select",
    "w-full rounded-xl border border-stone-200 bg-white p-3 text-sm outline-none focus:bg-teal-50",
  );
  select.setAttribute("aria-label", "选择模型实验");
  for (const model of data.models) {
    const option = el(
      "option",
      "",
      `${model.metrics.model} · ${model.metrics.mode} · ${model.name}`,
    );
    option.value = model.name;
    select.append(option);
  }
  select.value = "plus-multi-01";
  box.append(select);
  const content = el("div", "space-y-3");
  box.append(content);
  const render = () => {
    content.replaceChildren();
    const model = data.models.find((m) => m.name === select.value),
      audit = data.audit.find((a) => a.name === model.name);
    content.append(
      el(
        "p",
        "rounded-xl bg-teal-50 p-4 text-sm leading-7",
        model.workflow.summary,
      ),
    );
    if (audit)
      content.append(
        el(
          "p",
          "text-sm text-stone-500",
          `核心动作返回 ${audit.core_actions_returned} / 预期 ${audit.core_actions_expected}；文本/顺序/行号不匹配 ${audit.sequence_text_line_mismatches.length}；无效引用 ${audit.invalid_references.length}。这是有限核对，不是完整正确率。`,
        ),
      );
    model.workflow.steps.forEach((step, index) => {
      const card = el(
        "article",
        "rounded-xl border border-stone-200 bg-white p-5",
      );
      card.append(
        el(
          "h3",
          "font-semibold",
          `${index + 1}. ${step.action} · ${step.source_app ?? "未知来源"} → ${step.target_app ?? "未指定"}`,
        ),
      );
      if (step.text)
        card.append(el("p", "mt-2 whitespace-pre-wrap text-sm", step.text));
      card.append(
        el(
          "p",
          "mt-2 text-xs text-stone-500",
          `行号 ${step.line ?? "未知"} · ${step.outcome ?? "未标注结果"} · ${step.uncertainty || "模型未标注不确定性"}`,
        ),
      );
      const links = el("div", "mt-3 flex flex-wrap gap-2");
      for (const id of step.evidence_ids ?? [])
        links.append(
          button(id, () => jump(String(id)), "bg-stone-100 font-mono text-xs"),
        );
      card.append(links, jsonBlock(step, "查看此步骤的模型原始字段"));
      content.append(card);
    });
    content.append(jsonBlock(model.workflow.unresolved, "模型保留的不确定项"));
  };
  select.addEventListener("change", render);
  render();
  return box;
}

export function renderFiles(data) {
  const box = el("div", "space-y-4");
  box.append(
    el("h2", "text-2xl font-semibold", "原始文件与可追溯信息"),
    el(
      "p",
      "text-sm leading-7 text-stone-600",
      "摘要在生成查看器时计算，用于比对文件是否改变；摘要本身不能证明采集真实性。代码页也是生成时的快照，后续项目修改不会自动更新此 HTML。",
    ),
  );
  for (const file of data.manifest) {
    const item = el("div", "rounded-xl border border-stone-200 bg-white p-4");
    item.append(
      el("p", "break-all font-mono text-sm", file.file),
      el(
        "p",
        "mt-2 break-all font-mono text-xs text-stone-500",
        `${file.bytes.toLocaleString()} bytes · SHA-256 ${file.sha256}`,
      ),
    );
    box.append(item);
  }
  box.append(
    jsonBlock(data.session, "录制会话时钟"),
    jsonBlock(data.finished, "采集完整性标记（不代表 workflow 完整）"),
    jsonBlock(
      data.interactions,
      "现有 Normalizer 的 41 条交互归纳（软件规则，不是 AI）",
    ),
  );
  return box;
}
