import test from "node:test";
import assert from "node:assert/strict";
import { lowerPlan } from "../support/observation_demo/conversation/lower-plan.mjs";
import { verifyFacts } from "../support/observation_demo/conversation/verify-facts.mjs";

test('微信组合节点必须对应同轮粘贴、Enter 和实际剪贴板正文',()=>{
  const evidence={timeline:[{id:'paste',ms:2,target:'wechat',kind:'keyboard_chord',keys:['LeftControl','V']},{id:'send',ms:3,target:'wechat',kind:'keyboard_chord',keys:['Enter']}],origin:100,targets:{wechat:{title:'微信'}},source:{samples:[{id:'sample-1',from_epoch_ms:101,clipboard_current:'逐行正文',ocr:[{text:'文件传输助手'}]}]}};
  const plan={summary:'完整传输',nodes:[{id:'doc',type_id:'window.wait_document',config:{query:'文档(文本 包含 "")'},inputs:{expected:''},resources:{window:'editor_window'},evidence_ids:[]},{id:'disk',type_id:'file.wait_text',config:{},inputs:{path:{input:'output_path'},expected:''},resources:{},evidence_ids:[]},{id:'chat',type_id:'demo.wechat_paste_send',config:{},inputs:{recipient:'文件传输助手',expected:'逐行正文'},resources:{},evidence_ids:['paste','send','sample-1']}]};
  assert.equal(verifyFacts(lowerPlan(plan),evidence).covered_events,2);
  plan.nodes[2].inputs.expected='其他正文';
  assert.throws(()=>verifyFacts(lowerPlan(plan),evidence),/实际剪贴板/);
  plan.nodes[2].inputs.expected='逐行正文';
  plan.nodes[2].evidence_ids=['paste','sample-1'];
  assert.throws(()=>verifyFacts(lowerPlan(plan),evidence),/send/);
});
const node = (id, keys, evidence) => ({
  id,
  type_id: "aql.press_keys",
  config: { platform: "uia", query: "文档()", keys },
  inputs: {},
  resources: { scope: "editor_window" },
  evidence_ids: [evidence],
});
test("编译层只展开格式，保留 AI 顺序、参数和证据", () => {
  const plan = {
    summary: "测试",
    nodes: [
      node("second", "Control+C", "key-2"),
      node("first", "Control+V", "key-1"),
    ],
  };
  const result = lowerPlan(plan);
  assert.deepEqual(
    result.workflow.scopes[0].nodes.map((n) => n.id),
    ["second", "first"],
  );
  assert.deepEqual(result.workflow.scopes[0].nodes[0].action.task.resources, {
    scope: "editor_window",
  });
  assert.equal(
    result.workflow.scopes[0].nodes[0].timeout_ms.value.value,
    10000,
  );
});
test("只引用证据而没有对应按键也会被拒绝", () => {
  const result = lowerPlan({
    summary: "测试",
    nodes: [node("wrong", "Control+C", "key-1")],
  });
  assert.throws(
    () =>
      verifyFacts(result, {
        timeline: [
          { id: "key-1", kind: "keyboard_chord", keys: ["LeftControl", "V"] },
        ],
      }),
    /key-1/,
  );
});
test("重复动作必须对应不同节点，不能按相同文本或按键合并", () => {
  const result = lowerPlan({
    summary: "测试",
    nodes: [
      {
        ...node("one", "Control+C", "key-1"),
        evidence_ids: ["key-1", "key-2"],
      },
    ],
  });
  assert.throws(
    () =>
      verifyFacts(result, {
        timeline: [
          { id: "key-1", kind: "keyboard_chord", keys: ["LeftControl", "C"] },
          { id: "key-2", kind: "keyboard_chord", keys: ["LeftControl", "C"] },
        ],
      }),
    /顺序错误/,
  );
});
