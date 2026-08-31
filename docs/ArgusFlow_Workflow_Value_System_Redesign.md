# ArgusFlow 工作流变量 / 输入 / 节点输出 / 表达式系统重构方案

> 目标仓库：`SLOE-debug/argusflow`  
> 分析基线：`main` 分支，2026-08-31  
> 重点：彻底收敛当前「输入字段 / 本次运行输入 / 变量 / 节点输出引用」的认知模型，让用户只需要理解一个概念：**工作流中的值（Values）**。

---

## 0. 结论先行

当前 ArgusFlow **不是缺少“变量系统”**，而是已经有一套相当完整的底层 Value Runtime，却把底层抽象直接暴露给了 UI：

- Workflow Input 已存在；
- Workflow Variables 已存在；
- Node Published Outputs 已存在；
- `ValueExpr` 已经统一支持 `literal / ref / expression`；
- Runtime 已经有 `input / vars / nodes / result` 四个只读表达式根；
- 已经使用受限 Rhai 做高级表达式求值；
- 已经有节点输出注册、JSON Pointer、CFG dominance 校验；
- 已经有 `Set Variables` 节点做运行期事务式赋值；
- 已经有 Monaco 表达式编辑器。

**真正的问题是产品层和编辑器层没有把这些能力组织成一个统一、可视、可发现的“值空间”。**

当前 UI 让用户依次理解：

1. “输入字段 JSON”是什么；
2. “本次运行输入 JSON”是什么；
3. “变量 JSON”又是什么；
4. 节点里为什么还要选“数据来源”；
5. “引用数据 / 上游节点 / 某节点 / 全部数据 / JSON Pointer”分别是什么意思；
6. 为什么变量是一个工作流级 JSON，同时又有一个“设置变量”节点；
7. 为什么节点输出看起来不像变量，却又可以在表达式里通过 `nodes[...]` 使用。

这套 UX 把**内部数据平面的实现细节**变成了用户必须学习的产品概念。

### 建议的核心改造

只保留一个面向用户的上层概念：

> **值（Value） = 输入参数 + 工作流变量 + 节点输出 + 系统值（未来）**

然后提供两个统一入口：

1. **工作流数据面板**：负责 CRUD、查看和赋值；
2. **值选择器 / 表达式编辑器**：所有节点统一用它消费值。

其中：

- “输入字段”改成可视化 CRUD；
- “本次运行输入”根据输入声明自动生成表单；
- “变量”改成可视化 CRUD；
- “节点输出”自动进入统一值目录，不需要用户手动声明；
- “查看结果”节点不再出现四层“数据来源”下拉，而是直接选一个值；
- 高级用户可切换到表达式模式，用现有 Rhai 进行类似：

```rhai
vars["变量A"] + vars["变量B"] + "硬编码字符"
```

或者：

```rhai
input["contact_name"]
    + " / "
    + nodes["wechat_verify_search_1"]["text"]
```

**不要引入 JavaScript `eval`。现有受限 Rhai 正好就是安全、可校验、可持久化的 eval-like 能力，应继续作为唯一 Runtime 表达式引擎。**

---

# 1. 现状代码分析

## 1.1 工作流 Schema 已经天然分成 Input 与 Variable

前端 `src/features/workflow/model/contracts.ts` 当前定义：

```ts
export type WorkflowDefinition = {
  schema_version: 8;
  id: string;
  name: string;
  inputs: WorkflowInputDefinition[];
  variables: JsonObject;
  permissions: WorkflowPermissions;
  nodes: WorkflowNodeContract[];
  edges: WorkflowEdgeContract[];
};
```

其中：

```ts
export type WorkflowInputDefinition = {
  key: string;
  value_type: 'text';
};
```

当前语义已经很清楚：

- `inputs`：运行时由调用者提供、启动后冻结；
- `variables`：工作流持久化的初始变量对象，运行时可以被变量节点更新。

所以 **后端数据模型并不需要为了这次 UX 重构推倒重来**。

---

## 1.2 当前“输入字段 / 本次运行输入”之所以奇怪，是因为 UI 直接编辑序列化格式

`src/features/workflow/inputs/useWorkflowInputs.ts` 当前把声明和值都转换成 JSON 字符串：

```ts
const [definitionsDraft, setDefinitionsDraft] = useState(
  JSON.stringify(DEFAULT_WORKFLOW_INPUTS, null, 2),
);

const [valuesDraft, setValuesDraft] = useState(
  JSON.stringify(DEFAULT_RUN_INPUT_VALUES, null, 2),
);
```

输入声明还要求用户直接写：

```json
[
  {
    "key": "contact_name",
    "value_type": "text"
  },
  {
    "key": "message",
    "value_type": "text"
  }
]
```

运行时又要另外写：

```json
{
  "contact_name": "崽崽",
  "message": "今日天气"
}
```

而 `WorkflowInspectorFields.tsx` 更是直接用三个通用 `JsonEditorSection`：

- 输入字段
- 本次运行输入
- 变量

因此截图里的“抽象且奇葩”不是偶发现象，而是组件架构本身导致的：

> **UI 不是在编辑业务对象，而是在编辑后端 JSON 序列化结果。**

这是这次重构最应该先消灭的东西。

---

# 2. 当前 Value Runtime 其实已经非常接近目标架构

## 2.1 `ValueExpr` 已经是统一值引用模型

前端和 Rust 都已经有相同语义：

```ts
export type ValueExpr =
  | { type: 'literal'; value: JsonValue }
  | { type: 'ref'; source: ValueSource; pointer: string }
  | { type: 'expression'; source: string };

export type ValueSource =
  | { type: 'workflow_input'; key: string }
  | { type: 'variable'; name: string }
  | { type: 'node'; node_id: string };
```

这意味着：

- 输入是值；
- 变量是值；
- 节点输出也是值；
- 高级表达式最终也是值。

**所以应该围绕 `ValueExpr` 做 UI，而不是再造一套“新变量运行时”。**

---

## 2.2 节点输出本来就已经是 Runtime Value Scope 的一级成员

`crates/argusflow-runtime/src/value_runtime/scope.rs` 已经构建：

```rust
RuntimeValueScope {
    input,
    variables,
    nodes,
    result,
}
```

高级表达式里的四个根分别是：

```text
input   -> 本次运行输入
vars    -> 工作流变量当前快照
nodes   -> 已执行节点的 Published Outputs
result  -> 当前节点原生结果，仅 output mapping 阶段存在
```

这实际上已经是一个非常合理的 Value Plane。

因此用户提出的：

> “将节点的输出也视为变量”

建议在产品层解释为：

> **节点输出与变量一起出现在同一个“值选择器”中。**

而不是在运行时真的把每个节点输出复制进 `variables`。

### 为什么不能物理复制到 `variables`

如果把：

```text
节点A.text
```

执行后再复制成：

```text
vars.nodeA_text
```

会引入：

- 两份状态；
- 来源丢失；
- 重命名同步；
- 节点重新执行后的覆盖问题；
- 分支路径可用性问题；
- 输出名称冲突；
- 调试时无法判断值究竟从哪里产生。

所以应该是：

> **统一“展示”，不统一“存储”。**

---

# 3. 当前“查看结果”节点的问题到底在哪里

`argus.debug` 本身很简单：

```ts
debug: {
  value: ValueExpr
}
```

也就是说，查看结果节点本来只需要回答一个问题：

> **你想看哪个值？**

但当前 `ValueExprFields.tsx` 把一个简单选择拆成：

```text
数据来源
  └─ 引用数据

数据来自
  └─ 上游节点

数据来自
  └─ 某个节点

读取内容
  └─ 全部数据 / 某输出 / JSON Pointer
```

这就是截图二里的四层下拉。

技术上没错，产品上非常不自然。

用户真正的操作目标不是：

> “我要构造一个 `ValueSource::Node + JSON Pointer`。”

而是：

> “我要看【确认搜索界面 → 文本】。”

因此应改成：

```text
输出内容
┌─────────────────────────────────────┐
│ 确认搜索界面 · 文本                 ▼ │
└─────────────────────────────────────┘

[ 使用表达式 ]
```

一个下拉完成。

下拉内部按组展示：

```text
流程输入
  contact_name
  message

工作流变量
  search_keyword
  greeting

节点输出
  确认搜索界面
    文本
  读取联系人信息
    姓名
    ID
```

选择后内部仍然写成现有结构：

```json
{
  "type": "ref",
  "source": {
    "type": "node",
    "node_id": "wechat_verify_search_1"
  },
  "pointer": "/text"
}
```

**用户不需要知道这个 JSON 存在。**

---

# 4. 推荐的新产品概念：Workflow Value Space

## 4.1 用户只面对四类值

建议统一命名为“工作流数据”或“值”。

### A. 输入参数 Input

特点：

- 由工作流外部提供；
- 运行开始后只读；
- 必须声明；
- “本次运行输入”只是它在当前 Run 的具体取值。

例如：

```text
contact_name = "崽崽"
message      = "今日天气"
```

---

### B. 工作流变量 Variable

特点：

- 工作流设计时定义初始值；
- 每次运行复制一份；
- 可被“设置变量”节点修改；
- 生命周期只属于本次运行。

例如：

```text
retry_count = 0
search_text = ""
enabled     = true
```

---

### C. 节点输出 Node Output

特点：

- 自动产生；
- 只读；
- 由 Node Definition / Published Outputs 决定；
- 只有生产节点先成功执行后才存在；
- 在编辑器中和 Input / Variable 一样可以被选择。

例如：

```text
确认搜索界面.文本
执行命令.stdout
执行命令.exit_code
```

---

### D. 系统值 System Value（预留）

以后可以自然加入：

```text
run.id
run.started_at
workflow.id
workflow.name
```

当前版本不必实现，但统一 Value Space 后增加它会非常容易。

---

# 5. 新 UI：工作流数据面板

建议从工作流右侧 Inspector 中移除三个 JSON textarea：

```text
输入字段
本次运行输入
变量
```

改成一个独立的 **“工作流数据”** 区域。

可以放在：

- Workspace 下方 Dock；
- 或右侧 Inspector 的一级 Tab；
- 或画布顶部工具栏打开的 Drawer。

推荐用 Workspace Dock，因为变量属于工作流整体，而不是当前选中的节点。

---

## 5.1 面板结构

```text
┌──────────────────────────────────────────────────────────────┐
│ 工作流数据                                                   │
├──────────────────────────────────────────────────────────────┤
│ [输入参数 2] [工作流变量 3] [节点输出 6]                    │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│ 名称            类型      默认/当前值          操作           │
│ contact_name    文本      崽崽                 编辑 删除      │
│ message         文本      今日天气             编辑 删除      │
│                                                              │
│                                        [+ 添加输入参数]      │
└──────────────────────────────────────────────────────────────┘
```

---

# 6. 输入参数 CRUD

## 6.1 不再直接编辑 JSON

当前：

```json
[
  {
    "key": "contact_name",
    "value_type": "text"
  }
]
```

改成：

```text
名称          类型
contact_name  文本
```

按钮：

```text
+ 添加输入参数
```

编辑 Drawer / Inline Row：

```text
参数名称   [ contact_name ]
类型       [ 文本 ▼ ]
```

因为 schema v8 目前只支持 `text`，第一版甚至可以：

- 类型显示为“文本”；
- 下拉暂时 disabled；
- 不需要为了 UI 先扩后端类型。

---

## 6.2 本次运行输入不再是一个独立 JSON 概念

点击“运行”时，根据声明自动生成表单：

```text
运行工作流

联系人名称
[ 崽崽                         ]

消息
[ 今日天气                     ]

                         [取消] [运行]
```

对应内部数据仍然是：

```json
{
  "values": {
    "contact_name": "崽崽",
    "message": "今日天气"
  }
}
```

但用户不应该手写它。

### 高级模式

保留：

```text
[ 高级：JSON 编辑 ]
```

展开后才出现 JSON。

它的定位应该是：

- 批量复制；
- 调试；
- 开发者；
- API 对齐。

而不是默认 UX。

---

# 7. 工作流变量 CRUD

当前 `variables: JsonObject` 已经足够支持第一阶段可视化 CRUD。

例如内部：

```json
{
  "keyword": "",
  "retry_count": 0,
  "enabled": true
}
```

UI：

```text
┌────────────────────────────────────────────────────────────┐
│ 名称           当前初始值                 类型              │
├────────────────────────────────────────────────────────────┤
│ keyword        ""                         文本              │
│ retry_count    0                          数字              │
│ enabled        true                       布尔              │
│ payload        { ... }                    JSON              │
└────────────────────────────────────────────────────────────┘
```

操作：

```text
+ 新建变量
编辑
复制名称
删除
```

---

## 7.1 第一阶段不要急着修改 schema

可以直接使用：

```ts
variables: JsonObject
```

CRUD 操作本质就是：

```ts
function setWorkflowVariable(name: string, value: JsonValue) {
  setMetadata({
    variables: {
      ...variables,
      [name]: value,
    },
  });
}
```

删除：

```ts
function deleteWorkflowVariable(name: string) {
  const next = { ...variables };
  delete next[name];
  setMetadata({ variables: next });
}
```

因此：

> **这部分重构可以完全不修改 Rust Runtime。**

---

## 7.2 类型先由 JSON 值推断

当前 Runtime 明确允许变量运行时改变 JSON category。

现有测试甚至专门验证：

```text
number -> object
```

是允许的。

所以 MVP 不应该突然加“变量强类型”并破坏兼容性。

UI 中的类型可以只是显示：

```ts
function inferValueType(value: JsonValue) {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'array';
  return typeof value;
}
```

后续如果真的需要强类型变量，再单独升级 schema v9。

---

# 8. “设置变量”节点应该保留，但角色必须改变

当前项目里有两件不同的事：

### 工作流变量定义

```json
"variables": {
  "foo": 1
}
```

### 运行时变量赋值

```text
Set Variables Node
foo = expression
```

这两个概念本身没有问题。

问题是现在“设置变量”节点允许用户直接自由输入变量名：

```text
变量名 [____________]
```

这会让人误以为变量是在节点里“创建”的。

---

## 8.1 改成声明与赋值分离

### 工作流数据面板

负责：

```text
声明变量
初始化变量
删除变量
重命名变量
```

### 设置变量节点

只负责：

```text
给已经声明的变量赋值
```

UI：

```text
设置变量

变量
[ retry_count ▼ ]

值
[ 选择值 / 输入常量 / 表达式 ]

+ 添加赋值
```

变量名不再自由输入，而是下拉选择。

这会让 mental model 变成非常经典的：

```text
声明变量 -> 使用变量 -> 设置变量
```

---

## 8.2 建议补上“未声明变量”校验

当前 Rust 对变量引用基本只检查：

```text
name != empty
```

而 `Set Variables` 也只检查名称非空和同节点内唯一。

如果要建立真正的可视化变量 CRUD，建议新增规则：

> `ValueSource::Variable{name}` 必须存在于 `workflow.variables`。

以及：

> `Set Variables` 的 assignment name 必须已经声明。

否则用户从变量中心删除一个变量后，旧节点会继续保留隐式字符串引用。

推荐产生明确诊断：

```text
变量 'retry_count' 未声明
```

第一版如果担心旧工作流兼容，可以：

1. 加载旧 workflow 时把所有历史变量引用收集出来；
2. 对不存在的变量显示“未声明（Legacy）”；
3. 提供“一键加入工作流变量”。

---

# 9. 统一变量/值选择器：整个重构的核心组件

建议新建：

```text
ValuePicker
```

它统一负责选择：

- 工作流输入；
- 工作流变量；
- 节点输出。

而不是当前：

```text
ValueExprFields
  -> 数据来源
  -> 来源类别
  -> 节点
  -> 输出
  -> JSON Pointer
```

---

# 10. 前端统一 Symbol Registry

建议新增：

```text
src/features/workflow/values/workflowSymbols.ts
```

定义一个纯编辑器层模型：

```ts
export type WorkflowSymbol =
  | {
      id: `input:${string}`;
      kind: 'workflow_input';
      name: string;
      label: string;
      valueType: 'text';
      available: true;
    }
  | {
      id: `variable:${string}`;
      kind: 'variable';
      name: string;
      label: string;
      valueType: 'json';
      available: true;
    }
  | {
      id: `node:${string}:${string}`;
      kind: 'node_output';
      nodeId: string;
      outputName: string;
      nodeLabel: string;
      label: string;
      valueType: 'text' | 'json';
      available: boolean;
      unavailableReason?: string;
    };
```

注意：

> `WorkflowSymbol` **只存在于前端编辑体验层**。

它不是新的后端状态模型。

---

## 10.1 Symbol -> ValueExpr

```ts
export function symbolToValueExpr(symbol: WorkflowSymbol): ValueExpr {
  switch (symbol.kind) {
    case 'workflow_input':
      return {
        type: 'ref',
        source: {
          type: 'workflow_input',
          key: symbol.name,
        },
        pointer: '',
      };

    case 'variable':
      return {
        type: 'ref',
        source: {
          type: 'variable',
          name: symbol.name,
        },
        pointer: '',
      };

    case 'node_output':
      return {
        type: 'ref',
        source: {
          type: 'node',
          node_id: symbol.nodeId,
        },
        pointer: `/${escapePointerToken(symbol.outputName)}`,
      };
  }
}
```

这一步非常关键：

> **UI 是新体验，持久化格式仍然是现有 ValueExpr。**

这样风险最低。

---

# 11. 节点输出自动进入 Value Space

项目已经有：

```ts
getNodeValueOutputs(data)
```

它会合并：

- Native Outputs；
- `outputBindings` 自定义 Published Outputs；
- 被覆盖的 Native Outputs。

这其实正好就是节点输出目录的数据源。

所以：

```ts
function buildNodeOutputSymbols(nodes) {
  return nodes.flatMap(node =>
    getNodeValueOutputs(node.data).map(output => ({
      ...
    }))
  );
}
```

即可。

**不要重新维护第二份“节点输出变量定义”。**

---

# 12. 节点输出可用性：必须尊重执行图，不是“全局都能读”

用户想把节点输出像变量一样选择，这是对的。

但不能做成：

> 所有节点都可以无条件读任意其它节点的输出。

例如：

```text
       ┌─ A ─┐
Start ─┤     ├─ Merge -> C
       └─ B ─┘
```

如果 C 读取 A 的结果，但实际执行走了 B 分支，则 A 根本没有执行。

后端当前已经有正确的 CFG dominance 校验，并且会产生：

```text
reference_not_dominating
```

语义是：

> 生产节点必须在所有到达消费节点的路径上先执行。

这个机制必须保留。

---

## 12.1 UI 建议

值选择器默认只把“保证可用”的节点输出放到“可选”区域：

```text
节点输出
  ✓ 确认搜索界面 · 文本
  ✓ 读取联系人 · 姓名
```

其它输出放到：

```text
其它节点输出
  ⚠ 分支A结果 · 文本
    并非在所有执行路径上可用
```

默认禁用，或者允许选择但立即显示错误。

推荐第一版直接禁用，降低用户困惑。

---

# 13. “查看结果”节点新设计

当前：

```text
数据来源  [引用数据]
数据来自  [上游节点]
数据来自  [节点A]
读取内容  [全部数据]
```

改为：

```text
查看结果

输出值
┌─────────────────────────────────────────┐
│ 确认搜索界面 · 文本                   ▼ │
└─────────────────────────────────────────┘

[ 使用表达式 ]
```

如果选择整个节点对象，也可以在节点分组下面放：

```text
确认搜索界面
  整个结果对象
  文本
```

内部：

```json
{
  "type": "ref",
  "source": {
    "type": "node",
    "node_id": "..."
  },
  "pointer": ""
}
```

仍保持兼容。

---

# 14. 高级表达式：不要造 `eval`，直接把现有 Rhai 做好

项目当前 Rust Runtime 已经显式：

```rust
.disable_symbol("eval")
.disable_symbol("print")
.disable_symbol("debug")
```

并设置了：

- 最大 operation；
- 最大表达式深度；
- 最大调用层级；
- 最大字符串；
- 最大数组；
- 最大 map；
- 禁止循环；
- 禁止匿名函数；
- 禁止 statement expression。

这是正确的安全设计。

所以产品文案可以说：

```text
高级表达式
```

用户体验可以像 eval，但底层继续使用受限 Rhai。

---

## 14.1 用户要求的拼接已经天然可以表达

用户希望：

```text
变量A + 变量B + '硬编码字符'
```

推荐正式语法：

```rhai
vars["变量A"] + vars["变量B"] + "硬编码字符"
```

节点输出：

```rhai
nodes["node_a"]["text"] + " - " + nodes["node_b"]["text"]
```

混合输入：

```rhai
input["contact_name"]
    + "："
    + nodes["read_message"]["text"]
```

如果类型不确定：

```rhai
str(vars["变量A"])
    + str(nodes["node_b"]["value"])
    + "硬编码字符"
```

---

# 15. 高级表达式编辑器的 UX 应补齐

当前 `runtimeExpressionLanguage.ts` 的动态补全主要做了：

- `input`
- `vars`
- `nodes`
- `result`
- `str`
- `json`
- `get`
- 每个节点和 Published Output

但没有把实际的：

```text
input["contact_name"]
vars["keyword"]
vars["retry_count"]
```

作为完整候选直接提供。

建议把：

```ts
setRuntimeExpressionSuggestions(modelUri, nodes)
```

改成：

```ts
setRuntimeExpressionSuggestions(
  modelUri,
  nodes,
  workflowInputs,
  workflowVariables,
)
```

补全列表：

```text
输入
  input["contact_name"]
  input["message"]

变量
  vars["keyword"]
  vars["retry_count"]

节点输出
  nodes["wechat_verify_search_1"]["text"]

函数
  str(...)
  json(...)
  get(...)
```

这样用户甚至不需要记语法。

---

# 16. 推荐：简单模式 + 高级模式，而不是三种“数据来源模式”

当前 `ValueExprFields` 顶层：

```text
直接输入
引用数据
表达式
```

技术分类没错，但仍然偏底层。

推荐改为：

```text
值
[ 输入/选择一个值                                  ]

[ fx 高级表达式 ]
```

---

## 16.1 简单模式

单个控件支持：

### 直接输入

```text
"hello"
123
true
```

### 选择已有值

通过 `@` 按钮或下拉：

```text
@contact_name
@keyword
@确认搜索界面.文本
```

---

## 16.2 高级模式

点击：

```text
fx 高级表达式
```

打开现有 Workspace Monaco：

```rhai
input["contact_name"] + "：" + nodes["read_message"]["text"]
```

这样用户的学习路径变成：

```text
90% 用户：点选
10% 用户：表达式
```

而不是所有用户都先理解 `ValueSource`。

---

# 17. 不建议现在引入新的模板字符串 DSL

可能会想到设计：

```text
{{变量A}} + {{节点A.text}}
```

或者：

```text
${变量A}-${节点A.text}
```

第一版不建议。

因为项目已经存在：

- Rhai Parser；
- Runtime 编译；
- 类型转换；
- 错误诊断；
- 资源限制；
- Monaco Language；
- 表达式缓存 / plan。

再造 DSL 会引入：

- 第二个 parser；
- 第二套 escape 规则；
- 第二套 completion；
- 第二套错误位置；
- DSL -> Rhai 编译；
- 两种表达式持久化格式。

正确策略：

> **视觉选择器负责帮用户生成 Rhai，不再增加第三种表达式语言。**

---

# 18. 变量重命名：第一阶段需要明确约束

当前变量引用是：

```json
{
  "type": "variable",
  "name": "foo"
}
```

所以如果变量中心直接把：

```text
foo -> bar
```

改名，但不更新节点，引用会断。

输入参数同样如此。

---

## 18.1 推荐的 rename transaction

新增：

```ts
renameWorkflowVariable(oldName, newName)
```

必须一次事务更新：

1. `metadata.variables` key；
2. 所有节点 `ValueExpr::Ref(Variable)`；
3. 所有节点 expression source 中的直接文本引用——这一项无法安全靠字符串 replace；
4. Set Variables assignment name；
5. `output_bindings` 内的引用。

因为高级表达式是源码字符串，所以第 3 点是难点。

---

## 18.2 MVP 推荐策略

变量改名时：

```text
检测到有高级表达式直接引用 vars["foo"]。

[取消]
[仅修改变量名，表达式由我手动处理]
```

并列出引用位置。

不要做脆弱的全文字符串替换。

长期可以利用 Rhai AST / parser 做 token-level rewrite。

---

# 19. 第一阶段最值得做的“引用统计”

变量中心每一行增加：

```text
引用 3 处
```

点击后：

```text
输入联系人名称 · 输入值
条件判断 · 左值
查看结果 · 输出值
```

这会显著提升工作流可维护性。

同时删除变量时：

```text
变量 retry_count 被 3 个节点引用，不能直接删除。

[查看引用]
[强制删除并保留错误引用]
```

第一版推荐默认禁止删除。

---

# 20. 节点输出不应该允许“重命名”，只允许别名展示

节点输出的稳定身份应该保持：

```text
node_id + output_name
```

例如：

```text
wechat_verify_search_1 + text
```

用户可读展示：

```text
确认搜索界面 · 文本
```

不要用节点 label 作为引用 ID。

当前项目已经正确使用稳定 `node_id`，这一点应保留。

---

# 21. 推荐的 UI 信息架构

## 21.1 工作流级

```text
工作流
├── 基本信息
├── 工作流数据
│   ├── 输入参数
│   ├── 变量
│   └── 节点输出
└── 权限
```

---

## 21.2 节点级

任何能输入 `ValueExpr` 的字段都统一：

```text
字段名
[ 当前值 / 选择一个值                         ][@][fx]
```

其中：

- `@` = 值选择器；
- `fx` = 高级表达式。

---

# 22. 建议的新组件结构

```text
src/components/workflow/data/
├── WorkflowDataPanel.tsx
├── WorkflowDataTabs.tsx
├── WorkflowInputsTable.tsx
├── WorkflowVariablesTable.tsx
├── WorkflowNodeOutputsTable.tsx
├── WorkflowInputEditor.tsx
├── WorkflowVariableEditor.tsx
└── RunInputsDialog.tsx

src/components/workflow/value-editor/
├── ValueField.tsx
├── ValuePicker.tsx
├── ValuePickerGroup.tsx
├── ValueReferencePreview.tsx
└── ExpressionLauncher.tsx

src/features/workflow/values/
├── workflowSymbols.ts
├── workflowSymbolAvailability.ts
├── workflowValueExpressions.ts        // 已存在
├── runtimeExpressionLanguage.ts       // 已存在
└── workflowValueReferences.ts
```

---

# 23. `WorkflowSymbolRegistry` 设计

推荐不要存 registry，而是从当前工作流快照派生：

```ts
export type WorkflowSymbolRegistry = Readonly<{
  inputs: ReadonlyArray<WorkflowSymbol>;
  variables: ReadonlyArray<WorkflowSymbol>;
  nodeOutputs: ReadonlyArray<WorkflowSymbol>;
}>;

export function buildWorkflowSymbolRegistry(args: {
  inputs: ReadonlyArray<WorkflowInputDefinition>;
  variables: JsonObject;
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  edges: ReadonlyArray<WorkflowCanvasEdge>;
  consumerNodeId?: string;
}): WorkflowSymbolRegistry;
```

好处：

- 没有额外同步状态；
- Undo/Redo 自动一致；
- 节点删除后自动消失；
- output binding 修改后自动刷新；
- workflow variable CRUD 后自动刷新。

---

# 24. `ValuePicker` 的建议行为

```text
搜索值...
```

支持匹配：

- key；
- 变量名称；
- 节点 label；
- output label；
- node ID（高级用户）。

分组：

```text
输入参数
工作流变量
节点输出
```

每项显示：

```text
名称                     类型
contact_name             文本
retry_count              JSON
确认搜索界面 · 文本      文本
```

悬浮详情：

```text
来源：节点输出
节点：确认搜索界面
Node ID：wechat_verify_search_1
Output：text
```

---

# 25. 当前 JSON Pointer 的处理

当前结构化引用支持：

```text
pointer: RFC 6901
```

它对高级 JSON 数据仍然很有用。

但默认 UI 不应该要求普通用户理解 JSON Pointer。

推荐：

### 已知 Published Output

直接生成：

```text
/text
```

### JSON 对象内部路径

只有点击：

```text
高级：读取子字段
```

后才显示：

```text
JSON Pointer
[/user/name]
```

未来可以升级成可视化 JSON path browser。

---

# 26. 为什么节点输出要通过 Published Outputs 而不是原始 Runtime Result

项目当前已经把节点输出分成：

- Native Outputs；
- 用户 `output_bindings`；
- Published Outputs。

这是正确的封装边界。

统一变量选择器应该只展示：

```ts
getNodeValueOutputs(node.data)
```

不要直接把所有内部 `NodeOutcome` 字段暴露给用户。

否则节点实现一改，工作流就会碎。

---

# 27. `output_bindings` 其实也是这套 Value System 的重要一环

当前每个节点都可以：

```ts
output_bindings: Record<string, ValueExpr>
```

并在当前节点原生 `result` 快照上计算。

例如：

```text
原生结果：
  path
  bytes

自定义 Published Output：
  filename = get(result, "/path")
```

这意味着以后可以提供一个很自然的 UI：

```text
节点输出
  原生
    path
    bytes

  自定义
    filename = ...
```

它与本方案完全兼容。

---

# 28. 最小风险实施方案：Schema v8 不动

这是我最推荐的第一阶段。

## 保持不变

Rust / 持久化：

```text
WorkflowDefinition.inputs
WorkflowDefinition.variables
ValueExpr
ValueSource
RuntimeValueScope
Rhai evaluator
Published Outputs
output_bindings
RunInputs
```

## 主要改前端

把现有底层能力重新包装成：

```text
Workflow Data UI
Symbol Registry
Value Picker
Run Input Form
```

这样能用最小 Runtime 风险获得 80%~90% 的用户体验提升。

---

# 29. Phase 1：工作流数据 CRUD

## 修改

### `src/features/workflow/inputs/useWorkflowInputs.ts`

从：

```text
JSON Draft API
```

改成以结构化 API 为主：

```ts
addInput()
updateInput()
deleteInput()
setRunInputValue()
```

保留：

```ts
importInputsFromJson()
exportInputsAsJson()
```

作为 Advanced 工具。

---

### `src/features/workflow/studio/useWorkflowStudio.ts`

把：

```text
variablesDraft
variablesError
updateVariables(draft)
```

逐步替换为：

```ts
setVariable(name, value)
deleteVariable(name)
renameVariable(...)
```

如果需要高级 JSON：

```ts
replaceVariablesFromJson(...)
```

独立存在。

---

### `src/components/workflow/inspector/WorkflowInspectorFields.tsx`

删除默认的三个：

```text
JsonEditorSection
```

只保留：

- 基本信息；
- 权限；
- “打开工作流数据”入口。

---

# 30. Phase 2：统一 Symbol Registry + ValuePicker

新增：

```text
workflowSymbols.ts
```

把三类 source 统一成可显示 Symbol。

然后重构：

```text
ValueExprFields.tsx
```

不要再用“数据来源 / 数据来自 / 读取内容”的层级作为默认 UI。

---

# 31. Phase 3：优先改“查看结果”节点

这是最容易验证新模型的节点。

因为它的业务本质只有：

```ts
value: ValueExpr
```

建议先只给 Debug 节点使用新组件：

```tsx
<ValueField
  label="输出值"
  value={data.value}
  symbols={symbols}
  allowExpression
/>
```

用户验证满意后，再扩散到：

- Condition；
- UI Type Text；
- UI Set Value；
- Command arguments；
- Environment；
- Variable assignment；
- Output binding。

---

# 32. Phase 4：改“设置变量”节点

当前：

```text
变量名自由输入
```

改成：

```text
已声明变量下拉
```

值仍然走统一 `ValueField`：

```text
retry_count = [ 选择值 / 常量 / fx ]
```

---

# 33. Phase 5：表达式补全增强

修改：

```text
runtimeExpressionLanguage.ts
ExpressionEditor.tsx
```

让 Monaco 同时拿到：

```ts
{
  inputs,
  variables,
  nodes,
}
```

直接插入：

```rhai
input["contact_name"]
vars["retry_count"]
nodes["wechat_verify_search_1"]["text"]
```

---

# 34. Phase 6：变量声明一致性校验

Rust 侧补强：

### 变量引用

当前：

```text
只检查 name 非空
```

改成：

```text
name 必须存在于 workflow.variables
```

### Set Variables

assignment target 必须声明。

新增或复用 Validation Issue：

```text
invalid_variable_reference
undeclared_variable
invalid_variable_assignment
```

推荐新建明确 code：

```rust
UndeclaredVariable
```

这样前端可以做精确提示。

---

# 35. 不建议第一阶段做 Schema v9

以下需求才值得升级 v9：

```text
输入参数 display label
输入参数 description
required / optional
default value
secret
number / boolean / enum / object schema
变量强类型
变量描述
变量只读
变量敏感值
```

如果现在直接上 v9，会把一个主要是 UX 的问题扩大成：

- 前后端 schema migration；
- Runtime type system；
- backward compatibility；
- component input mapping；
- existing fixtures；
- run history snapshot；
- Tauri command compatibility。

没有必要。

---

# 36. 如果后续升级 Schema v9，推荐形状

```ts
type WorkflowInputDefinition = {
  id: string;
  key: string;
  label?: string;
  description?: string;
  value_type: 'text' | 'number' | 'boolean' | 'json';
  required: boolean;
  default_value?: JsonValue;
};

type WorkflowVariableDefinition = {
  id: string;
  name: string;
  label?: string;
  description?: string;
  initial_value: JsonValue;
};
```

但这应该是第二阶段甚至第三阶段。

---

# 37. Stable ID 的长期建议

当前 Input / Variable 都以名称作为身份：

```text
workflow_input.key
variable.name
```

这意味着 rename 会影响引用。

长期 v9 可以改成：

```text
input_id
variable_id
```

显示名独立。

例如：

```json
{
  "type": "variable",
  "id": "var_01H...",
  "name_snapshot": "retry_count"
}
```

但这是 schema 级架构升级，不应阻塞本次体验改造。

---

# 38. 新版“查看结果”最终效果示例

用户工作流：

```text
确认搜索界面
   ↓
选中搜索文字
   ↓
输入联系人名称
   ↓
打开联系人会话
   ↓
查看结果
```

选中“查看结果”：

```text
查看结果

输出值
[ 确认搜索界面 · 文本                          ▼ ]

────────────────────────────────

也可以组合多个值
[ fx 使用高级表达式 ]
```

点击下拉：

```text
搜索变量或节点输出...

流程输入
  联系人名称                  contact_name
  消息                        message

工作流变量
  greeting
  retry_count

节点输出
  确认搜索界面
    文本

  打开联系人会话
    confirmed
```

---

# 39. 高级表达式最终效果示例

点击 fx：

```rhai
input["contact_name"]
    + " | "
    + nodes["wechat_verify_search_1"]["text"]
    + " | "
    + str(vars["retry_count"])
```

右侧/下方提供实时辅助：

```text
input["contact_name"]                    流程输入 · contact_name
nodes["wechat_verify_search_1"]["text"]  确认搜索界面 · 文本
vars["retry_count"]                      工作流变量 · retry_count
```

---

# 40. 表达式预览建议

可以增加：

```text
表达式预览
```

设计时如果没有 Runtime 值：

```text
当前没有运行数据，无法预览。
```

完成一次运行后：

```text
最近一次运行：
"崽崽 | 搜索 | 0"
```

这不是 MVP 必须，但对自动化工作流调试非常有价值。

---

# 41. 运行后 Value Inspector：下一步非常值得做

统一 Value Space 后，可以自然增加一个运行时页签：

```text
当前运行值
```

显示：

```text
输入
  contact_name = "崽崽"
  message = "今日天气"

变量
  retry_count = 1

节点输出
  确认搜索界面
    text = "搜索"

  执行命令
    stdout = "..."
    exit_code = 0
```

这会比单独放很多 Debug 节点更强。

未来甚至“查看结果”节点都可以退化成专门的 Log/Debug Sink，而不是主要调试手段。

---

# 42. 对现有默认微信 Workflow 的直接改善

当前默认模板：

```ts
DEFAULT_WORKFLOW_INPUTS = [
  { key: 'contact_name', value_type: 'text' },
  { key: 'message', value_type: 'text' },
];

DEFAULT_RUN_INPUT_VALUES = {
  contact_name: '崽崽',
  message: '今日天气',
};
```

新 UI 直接渲染：

```text
输入参数

contact_name
本次运行：[崽崽]

message
本次运行：[今日天气]
```

用户甚至不需要知道：

```text
一个是 schema
一个是 value object
```

这才是产品应该做的抽象。

---

# 43. 推荐的实现优先级

| 优先级 | 改造 | 收益 | Runtime 风险 |
|---|---|---:|---:|
| P0 | 工作流 Input CRUD | 极高 | 无 |
| P0 | Workflow Variable CRUD | 极高 | 无 |
| P0 | Run Inputs 自动表单 | 极高 | 无 |
| P0 | 统一 Symbol Registry | 极高 | 无 |
| P0 | 查看结果单下拉 | 极高 | 无 |
| P1 | 所有 ValueExpr 使用统一 ValuePicker | 极高 | 低 |
| P1 | Monaco 输入/变量补全 | 高 | 无 |
| P1 | Set Variables 改声明变量下拉 | 高 | 低 |
| P1 | dominance-aware 可用性提示 | 高 | 无/低 |
| P2 | 变量引用统计 / 删除保护 | 高 | 无 |
| P2 | Runtime Value Inspector | 高 | 中 |
| P3 | Schema v9 typed inputs / stable ids | 中 | 高 |

---

# 44. 建议的代码改动清单

## 直接修改

```text
src/features/workflow/inputs/useWorkflowInputs.ts
src/features/workflow/studio/useWorkflowStudio.ts
src/features/workflow/values/runtimeExpressionLanguage.ts
src/features/workflow/values/workflowValueExpressions.ts

src/components/workflow/inspector/WorkflowInspectorFields.tsx
src/components/workflow/inspector/node-fields/ValueExprFields.tsx
src/components/workflow/inspector/node-fields/VariableNodeFields.tsx
src/components/workflow/inspector/node-fields/ExpressionEditor.tsx

src/components/workflow/workspace/WorkflowWorkspace.tsx
src/components/workflow/workspace/dock/WorkspaceDockPanel.tsx
```

## 新增

```text
src/features/workflow/values/workflowSymbols.ts
src/features/workflow/values/workflowSymbolAvailability.ts
src/features/workflow/values/workflowReferenceUsage.ts

src/components/workflow/data/WorkflowDataPanel.tsx
src/components/workflow/data/WorkflowInputsTable.tsx
src/components/workflow/data/WorkflowVariablesTable.tsx
src/components/workflow/data/WorkflowNodeOutputsTable.tsx
src/components/workflow/data/RunInputsDialog.tsx

src/components/workflow/value-editor/ValueField.tsx
src/components/workflow/value-editor/ValuePicker.tsx
src/components/workflow/value-editor/ValueReferencePreview.tsx
```

## Runtime 校验增强

```text
crates/argusflow-runtime/src/validation/validation_references.rs
crates/argusflow-runtime/src/builtin_nodes/variable.rs
```

---

# 45. `ValuePicker` 到现有 `ValueExpr` 的映射表

| 用户选择 | 内部 ValueExpr |
|---|---|
| 输入 `contact_name` | `Ref(WorkflowInput(contact_name), "")` |
| 变量 `foo` | `Ref(Variable(foo), "")` |
| 节点 A 整体结果 | `Ref(Node(A), "")` |
| 节点 A 的 text | `Ref(Node(A), "/text")` |
| 常量 `"abc"` | `Literal("abc")` |
| 组合表达式 | `Expression("...")` |

这张表说明：

> **本方案没有增加新的 Runtime 值种类。**

---

# 46. “全部数据”为什么应该降级成一个普通候选项

当前 UI 把：

```text
读取内容 -> 全部数据
```

放成单独一层。

新 UI 应该直接把它放到节点分组：

```text
确认搜索界面
  整个输出对象
  文本
```

用户选“整个输出对象”时：

```text
pointer = ""
```

就结束了。

---

# 47. 条件节点也会因此变得简单

当前 Condition 左值/右值同样可以使用 `ValueExpr`。

未来 UI：

```text
如果

[ @确认搜索界面.文本 ] [ 等于 ▼ ] [ "搜索" ]
```

或者：

```text
[ @retry_count ] [ 大于 ] [ 3 ]
```

不需要：

```text
数据来源 -> 引用数据 -> 变量 -> retry_count
```

---

# 48. UI 节点输入也会自然统一

比如“输入联系人名称”：

```text
输入文本
[ @contact_name ]
```

而不是让用户理解：

```text
引用数据
流程输入
contact_name
JSON Pointer
```

---

# 49. Command 节点也会自然统一

例如参数：

```text
程序
[ "python" ]

参数
1. [ "script.py" ]
2. [ @contact_name ]
3. [ fx: vars["prefix"] + input["message"] ]
```

底层仍然全部是 `ValueExpr`。

---

# 50. 统一 ValueField 后的最大架构收益

目前每种节点看起来都在“配置数据来源”。

重构后：

> 所有节点只是在“填一个值”。

这会把整个编辑器从：

```text
数据管线配置器
```

变成：

```text
自动化逻辑编辑器
```

认知成本会明显下降。

---

# 51. 需要避免的错误方案

## 51.1 不要把所有节点输出复制进 variables

原因：

- 重复状态；
- 来源丢失；
- 分支不安全；
- 命名冲突；
- 生命周期错误。

---

## 51.2 不要引入 JavaScript eval

当前 Rust 已经明确禁用动态 eval，这是正确的。

---

## 51.3 不要删除 `ValueExpr`

它已经是很好的抽象。

应该重做的是 UI。

---

## 51.4 不要为了 UI 立即升级整个 Schema

第一版可以 100% 基于 schema v8 完成。

---

## 51.5 不要继续让用户写 JSON Pointer

默认情况下应该由选择器生成。

---

## 51.6 不要把“上游节点”当成用户理解数据的主入口

用户想的是：

```text
我要这个值
```

不是：

```text
我要引用这个拓扑来源。
```

拓扑约束应该是系统自动验证的规则。

---

# 52. 测试计划

## 52.1 前端 Unit

### Workflow Inputs

```text
添加输入
删除输入
名称重复
空名称
运行值同步
```

### Workflow Variables

```text
添加变量
删除变量
编辑 JSON 值
变量重名
```

### Symbol Registry

```text
输入 -> Symbol
变量 -> Symbol
native output -> Symbol
custom output binding -> Symbol
节点删除 -> Symbol 消失
```

### ValueExpr conversion

```text
input symbol -> WorkflowInput Ref
variable symbol -> Variable Ref
node output -> Node Ref + pointer
whole node -> Node Ref + empty pointer
```

---

## 52.2 Graph Availability

至少覆盖：

```text
直线图
单分支
分支汇合
不可达节点
producer == consumer
producer 不支配 consumer
```

并与 Rust `reference_not_dominating` 的语义对齐。

---

## 52.3 Expression Editor

覆盖 completion：

```text
input["..."]
vars["..."]
nodes["..."]["..."]
str()
json()
get()
```

---

## 52.4 Runtime

继续保留现有：

```text
Rhai 禁止 eval
resource limits
变量事务 rollback
output mapping 原子性
node reference dominance
```

新增：

```text
undeclared variable reference
undeclared variable assignment
```

---

# 53. E2E 验收场景

## 场景一：创建 Input

用户：

1. 打开工作流数据；
2. 点击“添加输入”；
3. 输入 `contact_name`；
4. 点击运行；
5. 自动出现 `contact_name` 输入框；
6. 输入“崽崽”；
7. workflow 正常运行。

全程不出现 JSON。

---

## 场景二：创建 Variable

用户：

1. 工作流数据 -> 变量；
2. 新建 `prefix = "联系人："`；
3. 节点中打开 Value Picker；
4. 可直接选 `prefix`。

---

## 场景三：节点输出

用户：

1. 新增“查看结果”；
2. 打开“输出值”下拉；
3. 直接看到：
   `确认搜索界面 · 文本`；
4. 选择；
5. 运行；
6. Debug 输出正确。

全程不出现：

```text
引用数据
上游节点
读取内容
JSON Pointer
```

---

## 场景四：高级拼接

表达式：

```rhai
vars["prefix"]
    + input["contact_name"]
    + "："
    + nodes["wechat_verify_search_1"]["text"]
```

运行结果正确。

---

## 场景五：分支错误

分支 A 的输出在 Merge 后不是 guaranteed available。

Value Picker：

```text
⚠ 分支A · text
  不是所有执行路径都会产生该值
```

系统阻止或明确警告。

Rust Validator 仍能兜底。

---

# 54. MVP 验收标准

只有满足以下条件，才算这次重构真正完成：

- [ ] 默认 UI 不再让用户手写 Workflow Input Definition JSON；
- [ ] 默认 UI 不再让用户手写 Run Inputs JSON；
- [ ] 默认 UI 不再让用户手写 Workflow Variables JSON；
- [ ] Input 可以可视化新增、编辑、删除；
- [ ] Workflow Variable 可以可视化新增、编辑、删除；
- [ ] 运行前根据 Input 自动生成表单；
- [ ] 节点输出自动出现在统一 Value Picker；
- [ ] “查看结果”只需一次选择即可引用节点输出；
- [ ] Value Picker 能同时搜索 Input / Variable / Node Output；
- [ ] 高级表达式继续使用受限 Rhai；
- [ ] Monaco 能补全具体 Input / Variable / Node Output；
- [ ] 节点输出依然遵循 dominance 约束；
- [ ] Set Variables 选择已声明变量，不再鼓励自由造名字；
- [ ] 高级 JSON 仍可作为折叠后的开发者工具保留；
- [ ] schema v8 工作流无需迁移即可继续运行。

---

# 55. 推荐 PR 拆分

为了避免一次性大爆炸，建议拆成以下 PR。

## PR 1 — Workflow Data CRUD

```text
feat(workflow): add visual workflow inputs and variables editor
```

只改：

- Inputs CRUD；
- Variables CRUD；
- Run Inputs form；
- Advanced JSON。

不碰 ValueExpr。

---

## PR 2 — Unified Workflow Symbol Registry

```text
feat(workflow): introduce unified workflow value symbol registry
```

建立：

```text
input
variable
node output
```

统一目录。

不改 Runtime。

---

## PR 3 — Debug Value Picker

```text
feat(workflow): simplify debug node value selection
```

把“查看结果”改成一个 ValuePicker。

这是最重要的 UX 验证点。

---

## PR 4 — General ValueField

```text
refactor(workflow): unify ValueExpr editing through ValueField
```

逐步替代其它节点的旧 `ValueExprFields`。

---

## PR 5 — Expression Completion

```text
feat(workflow): autocomplete workflow inputs and variables in Rhai editor
```

---

## PR 6 — Declared Variable Validation

```text
feat(runtime): validate workflow variable declarations
```

Rust 校验与 Set Variables UX 一起收口。

---

# 56. 我认为最重要的架构决策

这次重构最重要的不是“加一个变量 CRUD 页面”。

而是明确下面这句话：

> **ArgusFlow 的用户层数据模型应该是一个统一的 Workflow Value Space；Input、Variable、Node Output 只是值的不同来源。**

后端可以继续保持：

```text
input
variables
nodes
result
```

这种清晰分离。

前端则应该把它们统一成：

```text
一个值选择器
一个表达式系统
一个数据浏览面板
```

这两层并不矛盾。

---

# 57. 最终建议

我建议直接按以下路线实施：

```text
保持 schema v8
保持 ValueExpr
保持 RuntimeValueScope
保持 Rhai
保持 Published Outputs
保持 dominance validation

↓

彻底重做前端 Value UX

↓

工作流数据 CRUD
统一 Symbol Registry
统一 ValuePicker
Run Inputs 表单
查看结果单下拉
Set Variables 声明式选择
Expression autocomplete
```

这样既能满足：

> “有一个可视化 CRUD 区域定义和赋值 workflow 级变量”

也能满足：

> “将节点输出视为变量，在查看结果节点直接下拉选择”

还能满足：

> “变量A + 变量B + '硬编码字符' 的高级拼接”

而且不需要为了实现这几个目标重写现在已经相当成熟的 Runtime Value Plane。

---

# 58. 一句话版本

**底层不要再造变量系统；上层把现有 Input / Variables / Published Node Outputs / Rhai Expression 收拢成一个真正的“工作流值系统”。**

这是当前 ArgusFlow 最低风险、最高收益、也最符合现有代码结构的改法。
