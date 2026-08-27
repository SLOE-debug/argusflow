# ArgusFlow 变量、表达式与流程运行时设计方案
> 仓库：`SLOE-debug/argusflow`
> 分析基线：`main`，2026-08-27
> 目标：不引入 Node.js，在现有 Rust Runtime 上实现 JS 风格动态值、节点输出引用、运行变量、表达式求值和节点自定义输出。
> 约束：不推翻 `PreparedNode / ResourceTable / ActionRouter`；核心方案少于 500 行。

## 0. 结论
不要再造第二套 Workflow Runtime。当前已有 `RunContext.workflow_inputs / node_outputs / variables`、`NodeOutcome.outputs`、`ValueExpr` 和 `ResourceTable`，真正需要的是把它们升级成统一 **Runtime Value Plane**。
```text
Workflow Input ─┐
Runtime Vars ───┼─> Structured Ref ─┐
Node Outputs ───┘                    ├─> serde_json::Value ─> Node Input
                         Rhai Expr ──┘                    └─> Output Mapping
```
我建议固定 6 条架构原则：
- 普通动态值统一使用 `serde_json::Value`。
- 真实资源继续使用 `ResourceRef + ResourceTable`，绝不塞进变量。
- `ValueExpr` 同时支持可视化引用与高级表达式。
- 节点可引用另一个节点的整个输出对象，或其中任意字段。
- 每个节点统一支持 `output_bindings`，允许自定义/覆盖公开输出。
- 表达式首选 Rhai；未来若必须 100% JavaScript 语义，再考虑 Boa，不需要 Node.js。

## 1. 当前代码现状
`crates/argusflow-runtime/src/run_context.rs` 已经是正确雏形：
```rust
pub struct RunContext { workflow_inputs: Map<String, Value>, node_outputs: HashMap<String, NodeOutcome>, resources: ResourceTable, variables: Map<String, Value> }
pub struct NodeOutcome { pub outputs: BTreeMap<String, Value>, pub resources: Vec<String> }
```
`crates/argusflow-core/src/value.rs` 当前：
```rust
pub enum ValueExpr { Literal { value: Value }, WorkflowInput { key: String }, NodeOutput { node_id: String, output: String }, Variable { name: String } }
```
所以问题不是“没有变量”，而是当前 `ValueExpr` 更像数据来源选择器，还不是通用动态表达式。
同时 `RunContext.variables` 还没真正贯穿 Condition/控制流，`NodeOutcome` 也缺统一输出重映射。

## 2. 当前最明显的三个缺口
第一，Debug 最终走 `resolve_text()`，对象/数组/数字不能直接输出；应改成 `resolve_value()` + 统一 formatter。
第二，前端 `ValueExprFields.tsx` 的节点输出引用需要手填“生产节点 ID + 输出端口”；普通用户不应该接触这些内部字符串。
第三，Condition 的分支选择仍读取 `workflow.definition.variables`，而不是本次运行的 `RunContext.variables`；这意味着所谓 runtime variables 尚未形成闭环。

## 3. 动态变量底层：直接用 JSON
你喜欢 JS 的核心其实不是 V8，而是“变量可持有任意普通值，并可在运行中换类型”：
```js
let x = 1; x = "hello"; x = { a: 1 }; x = [1, 2, 3]; x = null;
```
Rust 里现有 `serde_json::Value` 已覆盖 `null / bool / number / string / array / object`。
因此继续使用：
```rust
variables: Map<String, serde_json::Value>
```
允许 `vars.x` 从 number 变 string，再变 object；不需要变量声明、不做类型锁定。
但节点 API 自己仍可要求类型，例如 `Command.program` 最终必须是 string。也就是说：**变量动态，节点语义边界可强类型**。

## 4. Resource 与 Value 必须彻底分开
下面这些不能进入 `vars`：HWND、COM 对象、AppSession、BrowserSession、CDP WebSocket、native process handle。
继续保持：
```text
Value Plane    = JSON
Resource Plane = opaque runtime resource
```
真实句柄只在 `ResourceTable`；工作流定义里只保存 `ResourceRef`。
如果某个资源要被表达式使用，只能由节点显式导出普通 JSON metadata，不能把真实 handle 暴露给脚本。

## 5. ValueExpr V2
建议 schema v8 把现在四个来源型 variant 收敛成三个概念：
```rust
pub enum ValueExpr {
    Literal { value: Value },
    Ref { source: ValueSource, pointer: String },
    Expression { source: String },
}
pub enum ValueSource {
    WorkflowInput { key: String },
    Variable { name: String },
    Node { node_id: String },
}
```
`pointer` 直接使用项目已经在 Condition 中采用的 RFC 6901 JSON Pointer。
例：
```json
{"type":"ref","source":{"type":"node","node_id":"write-hot-search"},"pointer":"/path"}
```

## 6. 节点引用必须支持“整个输出对象”
我不建议强制所有节点只有一个叫 `output` 的端口。
更灵活的模型是：**每个节点天然公开一个 JSON Object：Published Outputs**。
例如文件写入节点公开：
```json
{"path":"C:\\Users\\me\\Desktop\\百度热搜.txt","bytes_written":1832,"success":true}
```
`pointer=""` 表示整个对象；`pointer="/path"` 表示路径；`pointer="/bytes_written"` 表示写入字节数。
这样 Command 可以自然保留 `stdout/stderr/exit_code`，Read 可以保留 `text/items/value`，文件节点可以保留 `path/bytes_written`。
如果用户喜欢一个单值 `output`，让用户通过后面的 `output_bindings` 自己创建即可。

## 7. 每个节点统一支持 output_bindings
不要让每一种节点重新实现“自定义输出”。直接把它放到通用 `WorkflowNode`：
```rust
pub struct WorkflowNode {
    pub id: String,
    pub position: Position,
    #[serde(flatten)] pub definition: NodeEnvelope,
    #[serde(default)] pub output_bindings: BTreeMap<String, ValueExpr>,
}
```
例如：
```json
{"output_bindings":{"output":{"type":"expression","source":"result.path"}}}
```
于是任何节点都能配置：`output = result.path`、`summary = result`、`count = result.count`。

## 8. 原始输出与公开输出分开
节点执行流程改为：
```text
PreparedNode.execute()
  -> Native NodeOutcome
  -> raw outputs 暂存为 result
  -> evaluate output_bindings
  -> merge 成 Published Outputs
  -> RunContext.record_outcome()
```
例如原生结果为 `{"path":"C:\\...\\百度热搜.txt","bytes_written":1832}`，配置 `output=result.path` 后，下游看到：
```json
{"path":"C:\\...\\百度热搜.txt","bytes_written":1832,"output":"C:\\...\\百度热搜.txt"}
```
为了最大灵活性，建议允许 custom binding 覆盖同名 native output；UI 对覆盖系统输出给警告即可。

## 9. output_bindings 必须顺序无关
所有输出映射必须读取同一个“映射前冻结快照”。
不要允许 `a=1` 后 `b=a+1` 隐式读取刚刚生成的 `a`，否则 JSON key 顺序会影响运行结果。
规则应是：每个 binding 都只能读取 `input / vars / nodes / result` 的同一份 snapshot。
所有 binding 全部成功后一次性 merge；任一个失败，整个节点不提交 Published Outputs。

## 10. 表达式作用域
P0 只暴露四个根对象：
```text
input  = 本次 Workflow Inputs
vars   = 当前 Runtime Variables 快照
nodes  = 已成功节点的 Published Outputs
result = 当前节点 Native Outputs，仅 output mapping 阶段存在
```
例：`input.order_id`、`vars.customer`、`nodes["write-hot-search"].path`、`result.path`。
持久化表达式必须使用稳定 `node_id`，不能使用节点显示名；UI 可以用 hover/decoration 显示中文名称，避免节点改名导致表达式失效。

## 11. 为什么同时保留 Ref 和 Expression
如果所有人都写 `nodes["foo"].stdout`，会失去现有 Runtime 已经做好的节点存在性、端口、类型和 CFG dominance 校验。
所以默认是 **Structured Ref**：选择节点 → 选择整个输出/字段；高级模式才是 **Expression**。
普通模式得到强静态验证，高级模式得到最大灵活性；二者最终都解析成 `serde_json::Value`。

## 12. Rust 中类似 JS eval/new Function：首选 Rhai
首选 Rhai，而不是 Node.js。
Rhai 是 Rust 原生嵌入式动态脚本语言，支持 `eval`、带 Scope 求值、compile AST、执行已编译 AST，也支持 serde bridge 和资源限制。
你的需求本质上是动态值、数组/对象、字符串处理、条件和节点结果组合，并不需要 npm、CommonJS、ESM、Node fs/process。
Cargo 可先增加：
```toml
rhai = { version = "1.25", features = ["serde", "sync"] }
```
权威数据仍然是 JSON，只在表达式边界做 `serde_json::Value -> Rhai Dynamic -> eval -> JSON`。

## 13. 如果未来必须 100% JavaScript
Plan B 是 Boa：纯 Rust ECMAScript 引擎，同样不依赖 Node.js。
如果未来明确要求真正 JS syntax / JS object / JS function semantics，可以把 evaluator 抽成 trait，再增加 `BoaExpressionEvaluator`。
但现在不要同时维护 Rhai + JS 两门语言；P0 用 Rhai 足够，而且更容易限制执行能力。

## 14. 不要把 rhai::Dynamic 当主数据模型
禁止把 `RunContext.variables`、`NodeOutcome.outputs`、Workflow inputs 全部改成 `rhai::Dynamic`。
正确边界是：
```text
Runtime Data = serde_json::Value
Expression Engine = 可替换 adapter
```
这样序列化、Tauri contract、Validator、事件、测试和节点类型系统都保持现状；以后换 Boa 也不动 Runtime 数据面。

## 15. Expression 必须是纯计算
P0 的 Expression 不允许文件 I/O、网络、Command、PowerShell、UIA、CDP、启动进程、修改 ResourceTable。
表达式只回答：“这个值应该是什么？”
所有副作用继续通过现有 Node / WorkflowPermissions / ResourceScheduler / ActionRouter 执行。
否则 Expression 会变成第二套万能系统脚本宿主，绕过权限、调度、事件、重试与资源生命周期。

## 16. Expression 编译一次，执行多次
不要节点每运行一次都 `engine.eval(source)` 重新 parse。
在 `prepare_workflow` 阶段把 `ValueExpr::Expression` 编译成 AST，执行阶段只做 `AST + Scope -> Value`。
建议 Runtime 内部有：
```rust
pub enum CompiledValueExpr {
    Literal(Value),
    Ref { source: CompiledValueSource, pointer: String },
    Rhai { ast: Arc<rhai::AST> },
}
```
这和当前 `NodeEnvelope -> NodeCompiler -> PreparedNode` 的“动态只发生在 prepare 边界”完全一致。

## 17. Rhai 必须加执行限制
至少限制 max operations、max expression depth、max string/array/map size、max variables。
不开 import/module resolver，不注册 file/network/process/native resource API。
这样 Expression 是一个“受控动态计算器”，不是第二个脚本运行环境。
P0 尽量只允许 expression，不鼓励完整 statement/script；状态修改通过专用 Variable Node 完成。

## 18. Runtime Variables：写入点必须显式
新增一个极简单节点：
```text
设置变量 / argus.variable.set
```
payload：
```rust
pub struct VariableAssignment { pub name: String, pub value: ValueExpr }
pub struct SetVariablesPayload { pub assignments: Vec<VariableAssignment> }
```
允许 `customer=nodes["read-customer"]`、`count=123`、`flag=true`、`temp=[1,2,3]`，下一次再 `temp="done"`。
不要允许任意参数表达式偷偷执行 `vars.x=...`，否则预览、重试、输出映射都会产生隐藏副作用。

## 19. Set Variables 必须事务式
一次 Set Variables 节点先冻结 scope，再计算所有 assignment；全部成功后一次性 commit。
如果变量 A 算成功、变量 B 失败，则 vars 完全不改变。
这会让重试语义清晰，也不会留下半个运行状态。

## 20. Condition 必须真正进入 RunContext
推荐 Condition payload 改成：
```rust
pub struct ConditionPayload { pub left: ValueExpr, pub operator: ConditionOperator, pub right: Option<ValueExpr> }
```
高级模式可以允许 `Expression -> bool`。
更关键的是 `PreparedNode::select_branch` 应接收 `&RunContext`，而不是静态 `workflow.definition.variables`。
这样 Condition 才能读取运行中刚写入的 vars、上游节点输出和 Workflow Input。

## 21. Debug 节点最终形态
Debug 不再声明 `ValueInput::text`，而是接受任意 JSON ValueExpr。
执行：`resolve value -> format_runtime_value -> ExecutionEvent::Log`。
格式：String 原样；Number/Bool/Null 用 JSON scalar；Array/Object pretty JSON。
因此用户可以选择“写入桌面百度热搜.txt → 整个输出对象”，也可以只选 `/path` 或自定义 `output`。

## 22. 前端 ValueExpr 编辑器 V2
顶层只显示三种模式：
```text
固定值
引用
表达式
```
引用模式内部：
```text
来源类型：[节点 / 运行变量 / 工作流输入]
节点：[上游节点下拉]
值：[整个输出对象 / 已知输出 / 自定义 JSON Pointer]
```
不要再出现“手填生产节点 ID”。
如果节点 descriptor 可枚举 outputs，UI 自动列出 `stdout/stderr/exit_code` 等；否则至少提供“整个对象 + JSON Pointer”。

## 23. Expression 编辑应进入中央 Workspace
延续 `docs/ArgusFlow_结构化内容编辑工作区重构方案.md` 的原则：Inspector 负责 scalar/summary，中央 Workspace 负责 AQL、PowerShell、CMD、JSON、Template、Expression。
Inspector 只展示：
```text
表达式
nodes["write-hot-search"].path
[编辑表达式]
```
中央编辑器 P0 提供 syntax highlight、compile error、`input/vars/nodes/result` completion、node id/output completion 和 hover 中文名即可。

## 24. 输出 descriptor
当前 `PreparedNode` 有 `value_output(name)`，长期建议增加可枚举描述：
```rust
pub struct ValueOutputDescriptor { pub name: String, pub value_type: ValueTypeId, pub label: String }
```
这样 Command 能公开 `stdout / stderr / exit_code`，Read 能公开 `text / items`。
前端 node registry 维护对应展示 metadata；普通用户直接选择，不猜字符串。

## 25. 静态验证策略
`ValueExpr::Ref` 继续使用当前 `validation_references.rs` 的强检查：producer 是否存在、是否支配 consumer、output 是否存在、类型是否兼容。
`ValueExpr::Expression` 不要一开始做完整静态类型系统，否则会变成再造 TypeScript。
P0 只做：prepare 时语法编译、runtime 实际求值、consumer 边界做结果类型检查。
以后可以从 Rhai AST 抽取 `nodes["id"]` 引用来做更多诊断，但不阻塞第一版。

## 26. 原子提交与 Retry
Output Mapping：所有 binding 成功才 record Published Outputs；任一失败则节点失败，不能写半份输出。
Set Variables：所有 assignment 成功才 commit vars；任一失败 vars 不变。
节点 retry 成功后，同 `node_id` 的 Published Outputs 覆盖旧结果，延续当前 `record_outcome` 语义。
因为 Expression 本身无副作用，retry 可以保持确定性。

## 27. Runtime Error
建议新增结构化错误：`ExpressionCompile`、`ExpressionEvaluation`、`ExpressionResultNotJson`、`ValuePointerNotFound`、`ValueTypeMismatch`、`VariableAssignmentFailed`、`OutputMappingFailed`。
至少携带 `node_id`、field/output name、message；有编辑器 source range 时一起返回。
不要统一压成一个模糊的 `script error`。

## 28. Schema 与模块改动
因为 `ValueExpr contract + WorkflowNode.output_bindings + Condition contract` 都会变化，建议明确 `schema v7 -> v8`。
Core：
```text
argusflow-core/src/value.rs    -> ValueExpr V2 / ValueSource
argusflow-core/src/workflow.rs -> WorkflowNode.output_bindings
```
Runtime：
```text
argusflow-runtime/src/value_runtime/{mod,evaluator,scope,formatter}.rs
run_context.rs                 -> variable get/set、whole outputs、JSON Pointer、scope snapshot
engine.rs                      -> native outcome -> output mapping -> published outcome -> runtime branch
builtin_nodes/utility.rs       -> Debug any JSON
builtin_nodes/control.rs       -> runtime ValueExpr Condition
builtin_nodes/variable.rs      -> Set Variables
validation_references.rs       -> Ref V2 + Expression compile validation
```
Frontend：
```text
contracts.ts                   -> ValueExpr V2
ValueExprFields.tsx            -> Literal / Ref / Expression
NodeInspectorFields.tsx        -> 通用 output bindings
ExpressionEditor.tsx
ExpressionFieldSummary.tsx
NodeOutputBindingsFields.tsx
```

## 29. AQL 不要复用成变量表达式语言
AQL 的职责继续是“跨 UIA / CDP / Vision 的 UI Query”；Value Expression 的职责是“运行时数据计算”。
例：
```text
AQL:        button where name == "保存"
Expression: nodes["read-order"].text
```
不要为了只有一个 DSL，把 AQL 扩成 arithmetic/variables/object transform；这会把两个完全不同的语义域绑死。

## 30. 典型流程
```text
[抓取百度热搜]
outputs: text, items
      ↓
[写入桌面百度热搜.txt]
content = Ref(抓取百度热搜, /text)
native outputs: path, bytes_written
custom output: output = result.path
      ↓
[调试输出]
source = 节点输出
node = 写入桌面百度热搜.txt
value = output
```
Debug 最终显示 `C:\Users\...\Desktop\百度热搜.txt`。
如果“值”选择“整个输出对象”，则直接显示 `{path, bytes_written, output}` 的完整 pretty JSON。

## 31. 高级表达式示例
```text
nodes["write-hot-search"].path
"文件已写入: " + nodes["write-hot-search"].path
vars.retry_count > 3
input.order_id
result.path
```
数组/Object 的计算能力直接交给 Rhai，不再设计 ArgusFlow 自己的 mini DSL。
P0 只注册极少量纯 helper，例如 `str(value)`、`json(value)`、`get(value, json_pointer)`。

## 32. 明确不做什么
不把整个 Runtime 改成 `rhai::Dynamic`；不把资源塞 vars；不让 Expression 直接调用系统副作用；不让每种节点自己实现一套输出映射；不强制所有节点只有一个 `output`；不复用 AQL 做通用表达式；不为了 JS 风格引入 Node.js。

## 33. 实施顺序
### PR 1：先把“节点值”真正用起来
Ref 支持整个节点输出与 JSON Pointer；前端改节点/输出下拉；Debug 支持任意 JSON；保留现有 dominance validation。此 PR 不需要 Rhai，风险最低但体验提升最大。
### PR 2：Rhai + 通用 output_bindings
增加 `ValueExpr::Expression`、prepare 编译 AST、`input/vars/nodes/result` scope、执行限制、`WorkflowNode.output_bindings`、原子输出映射、custom 覆盖 native。完成后已有大部分“JS 感”。
### PR 3：真正 Runtime Variables 闭环
增加 `argus.variable.set`、变量自由换 JSON 类型、assignment 事务 commit、Condition 改为 ValueExpr + RunContext、runtime variable inspector。

## 34. 必测场景
1. Ref 读取整个节点对象；2. Ref 读取 `/path`；3. Debug 输出 Object/Array/Number/Bool/Null；4. Variable 从 number 改 object；5. Expression 读取 input；6. Expression 读取 vars；7. Expression 读取 prior node outputs；8. output binding 读取 result；9. custom output 覆盖 native；10. mapping 顺序无关；11. mapping 失败不部分提交；12. Set Variables 失败不部分提交；13. structured ref 非支配引用仍被拒绝；14. expression syntax error prepare 阶段发现；15. operation/depth/collection limits 生效；16. expression 无法接触 ResourceTable；17. consumer 类型不匹配明确报错；18. retry 覆盖旧 Published Outputs。

## 35. 与现有 docs 的关系
本方案主要延续：
- `docs/ArgusFlow_App_Run_Node_Design.md`：控制流 + 数据流 + 资源生命周期；`RunContext / NodeOutcome / ValueExpr`；Value 与 Resource 分离。
- `docs/argusflow_architecture_review_79e00a3.md`：`NodeEnvelope -> PreparedNode`；动态扩展在 prepare 阶段冻结；不要让整个 Rust 热路径动态化。
- `docs/ArgusFlow_结构化内容编辑工作区重构方案.md`：Expression 属于 Structured Document，Inspector 只负责摘要。
- `docs/ArgusFlow_AQL_统一UI查询语言设计方案.md`：AQL 保持 UI Query 单一职责，不扩成 Runtime Expression DSL。

## 36. 最终架构
```text
                         RunContext
              ┌────────────┼────────────┐
              ▼            ▼            ▼
            input         vars         nodes
              └────────────┼────────────┘
                           ▼
                    Runtime Value Plane
                 ┌─────────┴─────────┐
                 ▼                   ▼
           Structured Ref         Rhai Expr
                 └─────────┬─────────┘
                           ▼
                  serde_json::Value
                    │             │
                    ▼             ▼
              Typed Node     Output Mapping
                 Input             │
                                   ▼
                          Published Outputs

ResourceRef -> ResourceTable -> App / Browser / UIA / CDP / ...
资源永远不进入 JSON Value Plane
```

## 37. 一句话方案
> **把现有 `ValueExpr + RunContext + NodeOutcome` 升级成统一 Runtime Value Plane：变量本身完全动态，普通用户用可静态验证的节点/字段引用，高级用户用受限 Rhai 表达式，每个节点统一支持自定义 Published Outputs，而真实 OS/Browser 资源继续严格留在 ResourceTable。**
最终得到：JS 风格动态变量 + 节点输出直接选择 + 整个节点结果传递 + 高级表达式 + 每节点自定义输出 + Rust 强类型执行核心 + 不需要 Node.js + 可控安全边界 + 未来可替换 Boa。
