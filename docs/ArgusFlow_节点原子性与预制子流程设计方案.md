# ArgusFlow 节点原子性、预制节点与可复用子流程方案
> 仓库：`SLOE-debug/argusflow`  
> 基线：`main`，2026-08-27  
> 目标：解决“节点该拆多细”“批量读取链接是否过度特化”“Application/Browser 如何抽象”，并引入可组合、可嵌套、可版本化的预制流程。  
> 原则：复用现有 schema v8、`NodeEnvelope / NodeTypeRegistry / PreparedNode / RunContext / ResourceTable / ValueExpr`，不重造第二套 Runtime。
---
## 0. 结论
你的方向基本对，但“原子性”不等于“越细越好”。ArgusFlow 应明确三层：
```text
Flow Component / 流程组件
“获取网页新消息”“发送消息给指定联系人”“百度热搜采集”
= 多节点，可双击进入，可版本化，可嵌套
        ↓
Node Preset / 节点预设
“点击”“读取文本”“输入文本”“访问网址”
= 单个底层原子节点的预填配置，不增加 Runtime 类型
        ↓
Semantic Primitive / 语义原子
Acquire App / Acquire Browser / Click / Extract / Navigate / Command / Transform
        ↓
Backend
UIA / CDP / OCR / Grounding / SendInput / Win32
```
核心规则：
> **Primitive 表达稳定业务语义或资源生命周期；Preset 负责易用性；Component 负责复用；Backend 负责实现细节。**
因此：
- `批量读取链接` 不适合继续作为底层 `UiOperation` 一等原子动作。
- `打开 Chrome 并访问 URL` 也混合了“获得 Browser 资源”和“导航”两个语义。
- 但不要继续拆成 `查进程 -> HWND -> Restore -> Focus -> FindElement -> Click`。
- 最合适的粒度是 **Semantic Atom（语义原子）**，不是 OS API 粒度。
- 你设想的“预制节点/预制流程”值得做，但内部应拆成 **Node Preset** 与 **Flow Component** 两类。
---
## 1. 当前代码已经有正确基础
当前 `WorkflowNode` 已经是开放 definition：`NodeEnvelope { type_id, version, payload }`，运行时通过 `NodeTypeRegistry -> NodeCompiler -> PreparedNode -> execute()` 编译和执行。
这意味着后端天然适合 Core Node、Preset、Component、Plugin Node，而不需要重新做一个中央大枚举。
Runtime 也已有：
- `RunContext / NodeOutcome / ValueExpr`
- `ResourceRef / ResourceTable`
- value/resource reference 与 dominance validation
- `output_bindings`
所以子流程输入输出不需要另造变量系统。
真正较封闭的是前端的 `WorkflowNodeData` 判别联合、`nodePaletteCatalog.ts` 和 `WORKFLOW_NODE_DEFINITIONS`；组件化主要从这里扩展。
---
## 2. `批量读取链接` 为什么确实太特化
当前代码直接存在 `UiOperation::CollectLinks` 和 `AutomationAction::CollectLinks`，它把下面这些行为一次写死：
```text
批量定位链接 + 读取可见标题 + 解析 URL + 拼 TSV 文本 + 输出 links 数组
```
Runtime 还专门要求 `CollectLinks` 必须允许 `BrowserCdp`。这说明它已经不是足够通用的跨场景 UI 原子，而是“网页链接列表复合采集”。
如果继续这种路线，很容易出现：
```text
CollectLinks / CollectImages / CollectButtons / CollectCards / CollectRows / ...
```
最终 `UiOperation` 会变成业务快捷功能仓库。
更合理的分层：
```text
通用 Extract Primitive
        ↓
“提取链接列表” Preset
        ↓
“百度热搜采集” Component
```
---
## 3. 原子性应该停在哪里
一个能力适合作为 Primitive，最好满足：
1. 名字是长期稳定语义，不包含具体网站/业务名词。
2. 输入契约在多个场景仍成立。
3. 输出可独立被别的节点消费。
4. 错误边界清楚。
5. 重试语义清楚。
6. 资源生命周期清楚。
7. 同一语义可换 Backend 时，不把 Backend 暴露给用户。
适合 Primitive：`Acquire Application / Acquire Browser / Navigate / Click / SetValue / GetText / Extract / Transform / Command / Condition`。
更适合 Component：`获取百度热搜 / 登录 ERP / 发送企业微信消息 / 导出日报 / 获取网页新消息`。
应留在 Runtime/Planner：`Resolve HWND / RestoreWindow / FindElementHandle / Resolve CDP target / 等 selector ready / UIA stale retry`。
一个实用判断：
> **拆出来的子动作，用户会不会在别的流程里单独想消费它的结果？**
如果不会，大概率不应该成为普通画布节点。
---
## 4. 不建议把 Click 拆成 Find + Click
不要默认设计 `Find(target) -> ElementHandle -> Click(handle)`，因为 UIA element、CDP node、视觉目标不是同一种稳定资源，element handle 还很容易 stale。
正确边界仍然是：
```text
Click(target) / GetText(target) / SetValue(target) / Extract(target-set)
```
Planner 内部再做 `AQL -> PreparedPlan -> UIA/CDP/Vision -> materialize -> execute`。
同理，当前 docs 已明确：为了让当前 UI 节点可执行而等待目标出现，属于节点 `target_wait`；只有真正业务上的固定暂停才使用 Delay。
这就是“不要拆得太碎”的止损线。
---
## 5. 推荐 Primitive 层
| 类别 | 建议 Primitive |
|---|---|
| Control | Start、End、Condition、Loop/ForEach、Delay、Retry、Component Instance |
| Resource | Acquire Application、Acquire Browser |
| UI | Click、SetValue、GetText、GetValue、Extract、Select、Toggle、Hotkey、Scroll |
| Browser | Navigate、Reload、NewPage、ClosePage |
| System | Command、File Read、File Write、HTTP |
| Data | Transform、Filter、Map、Project、Format |
Palette 可以显示很多快捷入口，但底层仍可统一到少量稳定 Runtime 类型。例如“点击/输入/读取/提取”都可继续保存为 `argus.ui` 的不同 operation。
---
## 6. 用通用 `Extract` 替代 `CollectLinks`
建议逐步引入：
```rust
UiOperation::Extract {
    target: AutomationTarget,
    cardinality: One | Many,
    fields: Vec<FieldProjection>,
}
```
`FieldProjection` 可以先支持：`Text / Value / Name / Property(name) / Attribute(name)`。
网页链接采集变成：
```text
Extract
target = AQL("...a...")
cardinality = Many
fields: title=Text, url=Attribute("href")
```
统一输出：
```json
{"items":[
  {"title":"热点 A","url":"https://example.com/a"},
  {"title":"热点 B","url":"https://example.com/b"}
]}
```
不要在基础 UI Operation 里额外输出 `title<TAB>url\r\n`；文本格式化交给 `Transform / Format`。
这样同一个 Extract 可复用到商品列表、订单表格、通知列表、联系人列表、搜索结果、网页链接、UIA ListView。
---
## 7. 通用 IR 不等于牺牲性能
可以保留 CDP 批量快速路径：
```text
Extract(Many,[Text,href]) -> Planner/Optimizer -> CDP bulk Runtime.evaluate
```
也就是：
> **Public IR 通用，Backend 可以专用并做 fusion。**
这样不会为了通用抽象退回低效逐元素执行，也不会把每种性能优化暴露成一个新业务节点。
---
## 8. `打开 Chrome 并访问 URL` 应拆语义
当前 `BrowserSpec` 同时包含 `executable_path / initial_url / launch_timeout_ms`，而 Browser 节点实际同时做“创建隔离 CDP 浏览器资源”和“首次导航”。
长期建议拆成：
```text
Acquire Browser -> Navigate(url)
```
但普通用户不必因此看到更碎的画布。Palette 可以提供：
```text
Preset/Component：打开网页
```
外部看一个节点，内部是 Acquire + Navigate。
---
## 9. Application 与 Browser 不建议强行合并成同一种 Runtime Resource
你的“应用资源都叫打开 APP”这个用户心智合理，但底层不宜粗暴合并。
Desktop Application：`Attach existing / Start process -> unique HWND -> AppSession`。  
Managed Chromium：`Launch/Attach CDP-enabled Chromium -> endpoint -> page target -> BrowserSession`。
关键区别：
> **已经运行的普通 Chrome.exe 不等于可以恢复成 CDP BrowserSession。**
如果它不是 remote debugging 模式启动，仅凭“找到已有 exe”不能获得 DOM/CDP 能力。
推荐用户层：
```text
应用资源
├─ 桌面应用
└─ Chromium 浏览器
```
两者共享“获取资源、启动/连接策略、超时、清理”的 UX，但 Runtime 继续保留 `AppSession` 与 `BrowserSession` 强类型。
原则：
> **统一 UX，不牺牲 Runtime 类型。**
---
## 10. Chrome 应允许两种入口
普通桌面应用路径：
```text
Acquire Application(chrome.exe) -> AppSession -> UIA/Vision/SendInput
```
受管浏览器路径：
```text
Acquire Browser(launch_isolated_cdp) -> BrowserSession -> Navigate -> CDP operations
```
未来只有宿主明确提供已有 CDP endpoint 时，才支持 `attach_cdp_endpoint`。
不要把“恢复已有 Chrome 进程”和“恢复 CDP 会话”混成同一语义。
---
## 11. Node Preset 与 Flow Component 必须区分
### Node Preset
定义：**一个 Primitive 的参数预填与展示包装。**
例如：`点击 / 输入文本 / 读取文本 / 读取值 / 提取列表`，底层都可以还是 `argus.ui`。
Preset：
- 不增加 Runtime node type；
- 不需要双击进入子画布；
- 拖入后就是普通节点；
- 解决 Palette 可读性，而不增加 IR 复杂度。
这与你现有 `argus.ui + operation dropdown` 完全兼容。
### Flow Component
定义：**拥有显式输入/输出、内部包含节点图、可版本化复用的嵌套流程。**
例如：`获取网页新消息 / 发送消息给指定联系人 / 获取百度热搜 / 登录 ERP / 写入 CRM`。
Component：
- 主画布看起来是一个节点；
- 双击进入内部流程；
- 可以包含 Primitive、Preset、其他 Component；
- `A Component + B Component + 普通节点` 可以继续保存为 C Component。
这正是你说的 `A flow + B flow + 一些节点 = 新 flow`。
---
## 12. Component Definition 建议
建议独立定义，而不是简单塞一个完整 `WorkflowDefinition`：
```rust
struct FlowComponentDefinition {
    schema_version: u32,
    id: Uuid,
    version: String,
    name: String,
    inputs: Vec<ComponentValueInput>,
    outputs: Vec<ComponentValueOutput>,
    nodes: Vec<WorkflowNode>,
    edges: Vec<WorkflowEdge>,
    entry_node_id: String,
    exit_node_id: String,
}
```
P0 先限制：
```text
1 个 control input + 1 个 control output + N 个 value inputs/outputs
```
P1 再增加 resource ports 和 multiple control exits。
主流程中只保存组件引用，例如：
```json
{
  "type_id":"argus.component",
  "version":1,
  "payload":{
    "component_id":"web-news-reader",
    "component_version":"1.3.0",
    "inputs":{"url":{"type":"ref","source":{"type":"workflow_input","key":"url"},"pointer":""}}
  }
}
```
组件输出仍表现成普通 Node Outputs，如 `messages / count / latest_message`，因此现有 `ValueExpr::Ref / NodeOutcome / output_bindings` 可继续使用。
---
## 13. Component 必须版本锁定
不要让共享组件更新后静默改变已发布 Workflow。
推荐 instance pin exact version，例如 `web-news-reader@1.3.0`。发布 `1.4.0` 后，旧流程继续使用 `1.3.0`，Studio 只提示“可更新”，由用户查看 diff 后显式升级。
这是企业 RPA 可审计性的底线。
---
## 14. P0 Runtime 推荐“编译期展开”
当前 Runtime 已经有 DAG 校验、reference/dominance 校验、ValueExpr 编译、PreparedNode 与 ResourceTable。
所以 P0 不建议 `ComponentNode.execute() -> 再启动一个 WorkflowEngine`，而建议：
```text
Load Workflow
-> Resolve Component refs
-> Recursive expand
-> namespace inner node ids
-> rewrite refs/edges/bindings
-> build SourceMap
-> existing validate + prepare
-> existing WorkflowEngine
```
例如 `news_reader_1::extract_messages`。
优点：直接复用现有 `NodeTypeRegistry / PreparedNode / validation_graph / RuntimeValuePlan / ResourceTable / Scheduler / ExecutionEvent`。
一句话：
> **编辑器是嵌套的，第一版 Runtime 可以是扁平的。**
---
## 15. Component 展开必须带 SourceMap
需要维护：
```text
expanded_node_id <-> component_instance_id <-> inner_node_id <-> component_version
```
内部节点失败时，外部显示组件失败；双击组件后再显示具体内部红色节点。
P0 安全限制：
- 禁止递归组件；
- 最大嵌套深度 8；
- 必须 pin version；
- 内部仍是 DAG；
- 入口/出口唯一；
- 组件不能提升父 Workflow 权限。
---
## 16. 前端最小改法
P0 不必一次性插件化整个前端，只需：
```text
WorkflowNodeData 新增 kind=component
新增 ComponentNodeData
新增 ComponentCatalog
```
Palette 数据源改成：
```text
Core Node Catalog + Node Preset Catalog + Flow Component Catalog
```
长期再把 `WORKFLOW_NODE_DEFINITIONS` 的 metadata、inspector、palette、outputs 完全 registry-driven。
推荐 Palette：
```text
原子节点：流程 / 应用资源 / 界面 / 浏览器 / 系统 / 数据
预制节点：点击 / 输入 / 读取 / 提取列表 / 打开网页 / 写文件
流程组件：官方组件 / 工作区组件 / 我的组件
```
卡片加 `原子 / 预设 / 组件` Badge，避免用户不理解为什么有的节点可以双击进入。
---
## 17. “从选中节点创建组件”应该是一等功能
用户框选：
```text
Acquire Browser -> Navigate -> Extract -> Transform
```
右键“创建流程组件”，配置名称、输入端口、输出端口、版本、保存位置，完成后原地折叠成 `[获取网页新消息]`。
以后：
```text
组件 A + 组件 B + 普通节点 -> 再框选 -> 创建组件 C
```
这会自然形成数字员工“技能库”，而不是不断把业务逻辑硬编码进 Primitive。
---
## 18. 截图中的流程建议改成什么
当前：
```text
Start -> 打开 Chrome 并访问百度 -> 批量读取热搜标题和链接 -> 写文件 -> End
```
底层原子版：
```text
Start
-> Acquire Browser
-> Navigate(百度)
-> Extract(target=热搜条目, fields=title/href)
-> Transform(items->text)
-> Write File
-> End
```
普通用户不必天天看 6 个节点，可以保存为：
```text
Start -> [获取百度热搜] -> [写入文本文件] -> End
```
甚至继续封装为 `[采集并保存百度热搜]`。
外部越简洁，内部仍保持可审计、可展开、可复用。
---
## 19. “获取网页新消息 + 发给通讯软件联系人”的组合
组件 A：
```text
获取网页新消息(url) -> outputs: messages, latest
内部：Acquire Browser -> Navigate -> Extract -> Filter/Transform
```
组件 B：
```text
发送消息(contact,message)
内部：Acquire Application -> 搜索联系人 -> SetValue -> Click Send
```
主流程：
```text
Start
-> 获取网页新消息
-> Condition(messages not empty)
-> 发送消息(contact="xx", message=latest)
-> End
```
以后框选 A + Condition + B，可继续创建组件 `[网页消息转发器]`。
---
## 20. `CollectLinks` 迁移策略
Phase A：新增 `UiOperation::Extract`，保留 `CollectLinks` 兼容旧流程。  
Phase B：Studio 新建流程不再暴露“批量读取链接”，改为 Preset“提取链接列表”，实际生成 `Extract(fields=title,href)`。  
Phase C：老 workflow 加载/编译时把 `collect_links` 视为 migration/compiler sugar；内部 CDP 仍保留批量优化。
---
## 21. Browser 迁移策略
当前：
```text
BrowserSpec { executable_path, initial_url, launch_timeout_ms }
```
建议 v2：
```text
AcquireBrowserSpec { executable_path, acquire_mode, launch_timeout_ms, cleanup_policy }
BrowserOperation::Navigate { browser: ResourceRef, url: ValueExpr }
```
v1 在 compiler/migration 中视作 `Acquire Browser + Navigate(initial_url)`，不需要一次性破坏旧工作流。
---
## 22. 实现顺序
1. **P0-1：新增通用 `Extract`**，`CollectLinks` 降级为兼容 sugar/preset。
2. **P0-2：拆 Browser Acquire 与 Navigate**，Browser v1 继续兼容。
3. **P0-3：做 Node Preset**，Palette 显示点击/输入/读取/提取，但 IR 仍主要是 `argus.ui`。
4. **P0-4：做 Flow Component**：`ComponentDefinition / ComponentRegistry / argus.component / ComponentExpander / SourceMap`。
5. **P0-5：Studio Drill-down**：双击进入、Breadcrumb、公开输入输出、从选中节点创建组件、展开组件、版本更新。
6. **P1**：Resource ports、多 control output、组件断点、发布/市场、权限审计。
---
## 23. 主要代码改动位置
后端现有文件：
```text
crates/argusflow-core/src/automation.rs
crates/argusflow-core/src/browser.rs
crates/argusflow-core/src/workflow.rs
crates/argusflow-runtime/src/builtin_nodes/ui.rs
crates/argusflow-runtime/src/node_registry.rs
crates/argusflow-runtime/src/validator.rs
```
建议新增：
```text
crates/argusflow-core/src/component.rs
crates/argusflow-runtime/src/component_registry.rs
crates/argusflow-runtime/src/component_expander.rs
```
前端现有文件：
```text
src/features/workflow/contracts.ts
src/features/workflow/workflowModel.ts
src/features/workflow/workflowNodeDefinitions.ts
src/components/workflow/nodePaletteCatalog.ts
src/components/workflow/NodePalette.tsx
src/components/workflow/WorkflowCanvas.tsx
```
建议新增：
```text
src/features/workflow/componentModel.ts
src/components/workflow/ComponentNodeFields.tsx
src/components/workflow/ComponentBreadcrumb.tsx
src/components/workflow/componentCatalog.ts
src/components/workflow/nodePresetCatalog.ts
```
---
## 24. 建议拆出的 TODO
```text
COMPONENT-001 P1 FlowComponentDefinition + version
COMPONENT-002 P1 ComponentRegistry + pinned version resolve
COMPONENT-003 P1 compile-time expansion + ID namespace
COMPONENT-004 P1 ValueExpr/edge/binding rewrite
COMPONENT-005 P1 source map + nested run state
COMPONENT-006 P1 Studio drill-down/breadcrumb
COMPONENT-007 P1 从选中节点创建组件
COMPONENT-008 P1 组件更新提示与显式升级
COMPONENT-009 P2 resource ports
COMPONENT-010 P2 multiple control outputs
PRESET-001 P1 NodePreset catalog，不增加 Runtime node type
UI-EXTRACT-001 P1 通用 Extract 替代新建 CollectXxx
BROWSER-OP-001 P1 分离 Browser Acquire 与 Navigate
```
现有 `EDITOR-012`“子流程、节点分组、注释和可复用模板”建议拆成上面的可验收任务，并提升到近期主线。
---
## 25. 验收标准
- 新增业务快捷能力时，通常不用修改 Rust `UiOperation`。
- Palette 可以持续增加 Preset，而 Runtime 类型数量稳定。
- 任意连续流程可以保存成 Component。
- Component 可以包含 Component，但禁止递归。
- 主画布可折叠，双击可进入内部。
- Component 输入输出直接复用 `ValueExpr / NodeOutcome`。
- Component 不绕过 dominance、permissions、ResourceTable。
- 老 `collect_links` 工作流仍能运行。
- CDP 批量性能不因公共 IR 泛化明显下降。
- “打开浏览器”不再天然等于“导航某 URL”。
- 普通用户无需看到 UIA/CDP/OCR/FindElement/RestoreWindow 等内部实现。
---
## 26. 最终节点哲学
建议写进项目架构规范：
> **Primitive 负责可复用的稳定语义。**  
> **Preset 负责让 Primitive 好用、好找、好理解。**  
> **Component 负责把多个 Primitive / Preset / Component 封装成业务能力。**  
> **Backend 负责“怎么做”，而不是让用户选择底层技术细节。**
最后一条底线：
> **不要为了画布看起来简单，把业务组合硬编码进 Primitive；也不要为了追求原子性，把 Runtime 内部步骤暴露给用户。**
---
## 27. 本方案主要参考
```text
docs/ArgusFlow_App_Run_Node_Design.md
docs/ArgusFlow_变量与流程运行时设计方案.md
docs/ArgusFlow_节点内建等待与UI就绪同步方案.md
crates/argusflow-core/src/workflow.rs
crates/argusflow-core/src/automation.rs
crates/argusflow-core/src/browser.rs
crates/argusflow-runtime/src/node_registry.rs
crates/argusflow-runtime/src/validator.rs
crates/argusflow-runtime/src/builtin_nodes/ui.rs
src/features/workflow/contracts.ts
src/features/workflow/workflowModel.ts
src/features/workflow/workflowNodeDefinitions.ts
src/components/workflow/nodePaletteCatalog.ts
TODO.md
```
