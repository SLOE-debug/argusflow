# ArgusFlow UI 与目录结构重构方案
> 审查仓库：`SLOE-debug/argusflow`；分支：`main`；基线：`8ae0f0f6957e5bd8eb1b15f549eecbdd619a9ef3`  
> 目标：不改协议、不改算法、不换技术栈，用小步可回滚方式完成 UI 提重与前后端目录规整。  
> 参考：`AGENTS.md`、`README.md`、`docs/ArgusFlow_结构化内容编辑工作区重构方案.md`、`docs/ArgusFlow AQL 审计与重构方案.md`、`docs/argusflow_architecture_review_79e00a3.md` 等。

## 1. 结论
你的方向对，但有一个关键修正：**不要再新建第二个 `ui` 文件夹。**

当前已经有：
```text
src/components/ui/
  Checkbox.tsx Input.tsx Select.tsx Textarea.tsx
  PanelResizeHandle.tsx formControlStyles.ts index.ts monaco/
```

真正的问题是：
- `src/components/ui` 只覆盖少量表单，没有 Button/Dialog/Menu/Tooltip 体系。
- `src/components/workflow` 明显过平：Inspector、Node Fields、Workspace、Dock、Palette、Execution 全在一级。
- `src/features/workflow` 也开始堆积：模型/API/节点/表达式/组件编排混在一级。
- `src/flow` 是前端最扁平的一块：Canvas/Store/Routing/Interaction/Geometry 同级铺开。
- 后端不是整体都扁平：`argusflow-browser/src/cdp`、`argusflow-windows/src/uia|window|input` 已经规整。
- 后端最值得整理的是 `argusflow-runtime/src`；`argusflow-core/src` 只建议轻整理。

长期边界保持：
```text
UI primitive       -> src/components/ui
业务视觉组件        -> src/components/workflow
业务模型/API/状态    -> src/features/workflow
通用画布内核        -> src/flow
桌面适配            -> src-tauri
领域契约            -> argusflow-core
执行编排            -> argusflow-runtime
平台实现            -> browser/windows/vision/agent
```
这与现有 `AGENTS.md` 一致，不需要重新发明架构。

## 2. 为什么现在适合做
`docs/ArgusFlow_结构化内容编辑工作区重构方案.md` 已经把边界定成：
```text
Inspector -> 属性/摘要/进入编辑
Workspace -> AQL/脚本等结构化内容
```
源码也已经出现 `AqlFieldSummary`、`ScriptFieldSummary`、`StructuredFieldSummary`、`WorkspaceDockPanel`、`WorkspaceEditorHeader`、`WorkspaceStructuredEditor`、`useWorkspaceEditor` 等模块。

这些模块方向没问题，但继续被塞进 `src/components/workflow/` 根目录，所以现在是：**语义边界已经存在，物理目录没有表达出来。**

本次应做“目录归位 + UI primitive 提重”，不要顺手重写 Workspace/AQL/Runtime。

## 3. 紧凑 UI：直接标准化现有 26px 风格
你截图中的风格当前 `EditorPrimaryActions.tsx` 已经基本实现：
```text
高度 26px；字号 12px；图标约 12px；rounded-md；blue-600；Split 区 26px
```
`formControlStyles.ts` 也已有：
```text
compact = h-[26px] + text-[12px]
standard = h-8 + text-[12px]
```
所以不要做“大而全 Design System”，只把现有高密度桌面语言提升为基础组件契约。

推荐：
```text
src/components/ui/
  button/  -> Button, IconButton, SplitButton, buttonStyles
  form/    -> Input, Select, Textarea, Checkbox, FormField, formControlStyles
  overlay/ -> Dialog, ConfirmDialog, DropdownMenu, Tooltip
  layout/  -> PanelResizeHandle
  monaco/
  index.ts
```
明确不要再造 `src/ui`、`src/shared/components`、`src/common/components`。唯一通用 UI 层就是 `src/components/ui`。

## 4. Button 体系只做 3 个核心件
### Button
建议：
```ts
type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
type ButtonSize = 'compact' | 'standard';
```
视觉：
```text
compact: h-[26px] px-2.5 text-[12px] gap-1.5 icon 12~14px
standard: h-8 px-3 text-[12px]
```
统一处理 `hover / focus-visible / disabled / loading`，业务组件不再复制这套 class。

### IconButton
替代当前多处 `button + size-7 + rounded-[4px] + hover/focus` 的重复样式。建议 compact 26~28px、图标 14px。

### SplitButton
当前 `EditorPrimaryActions.tsx` 内部的私有 `SplitActionButton` 正是第一个该提重的组件：
```tsx
<SplitButton variant="primary" icon={Play} label="运行"
  onPrimaryClick={onRun} menu={...} />
```
“运行/发布”的视觉实现不应该属于 Workflow 业务层。

## 5. Dialog/Menu 不先引大型 UI 框架
当前项目没有 Radix/shadcn/Headless UI。ArgusFlow 是 Windows + WebView2，可以先薄封装原生 `<dialog>`。

`Dialog` 只负责：
```text
controlled open/close；Escape；backdrop；initial focus；
关闭后恢复焦点；header/body/footer；aria label
```
业务确认框用组合：
```tsx
<ConfirmDialog title="删除节点？" description="此操作无法直接撤销。"
  confirmText="删除" variant="danger" />
```
DropdownMenu/Tooltip 同理，不让 Workflow/Inspector/Workspace 各自写 backdrop、click-away、focus、Escape。

## 6. `src/components/workflow`：P0 目录重构
推荐：
```text
src/components/workflow/
  workspace/
    WorkflowWorkspace.tsx WorkflowCanvas.tsx WorkspaceStatusBar.tsx
    toolbar/ -> EditorPrimaryActions.tsx EditorToolbarControls.tsx
    dock/ -> WorkspaceDockPanel.tsx WorkspaceDockResizeHandle.tsx
             WorkspaceEditorHeader.tsx WorkspaceStructuredEditor.tsx
             structuredEditorTarget.ts useWorkspaceEditor.ts
  inspector/
    NodeInspector.tsx NodeInspectorFields.tsx WorkflowInspectorFields.tsx
    InspectorControls.tsx
    common/ -> StructuredFieldSummary.tsx AqlFieldSummary.tsx ScriptFieldSummary.tsx
    node-fields/ -> 所有 *NodeFields.tsx、ValueExprFields、NodeOutputBindingsFields、
                   CommandScriptField/Editor、ExpressionEditor、commandScript.ts
  palette/ -> NodePalette.tsx PaletteNavigation.tsx nodePaletteCatalog.ts
  canvas/ -> WorkflowNodeCard.tsx ComponentDrillDown.tsx
  execution/ -> ExecutionLog.tsx
  overview/ -> WorkflowOverview.tsx WorkflowTaskTable.tsx workflowTaskData.ts workflowStatus.ts
  index.ts
```

### Node 属性相关文件放哪？
固定放：
```text
src/components/workflow/inspector/node-fields/
```
判断规则只有一个：**这个组件是否只负责编辑某一类 Node 的属性？是就放这里。**

暂时不要每个节点再套一层目录；只有某个节点族稳定超过 3~4 个文件时再细分，避免“为了整齐制造五层目录”。

## 7. `src/features/workflow`：按领域能力分，不按页面布局分
建议：
```text
src/features/workflow/
  model/      -> contracts, workflowModel, workflowNodeDefinitions, defaultWorkflowTemplate
  api/        -> workflowApi
  studio/     -> useWorkflowStudio
  nodes/      -> nodePresetCatalog, workflowAction/Application/Browser/Command
  values/     -> workflowValueExpressions, runtimeExpressionLanguage, workflowResourceBinding
  components/ -> componentCatalog, componentCreation, reusableFlowContracts, useWorkflowComponents
  execution/  -> executionEventPresentation, useAqlInspection
  inputs/     -> useWorkflowInputs
  index.ts
```
原则不变：`features/workflow` 管模型/API/状态/领域转换，不承载具体页面视觉。

## 8. `src/flow`：P1，按内核子系统拆
当前 `src/flow` 职责其实清楚：业务无关画布内核；问题只是文件平铺。

建议：
```text
src/flow/
  canvas/      -> FlowCanvas*, FlowNodeView, FlowEdges, FlowMenu, FlowContextMenu
  interaction/ -> dragDrop, pointerGesture, useCanvasKeyboard/PointerInteractions/Size
  store/       -> store, createFlowStore, flowStoreTypes/Document/History/Selection
  routing/     -> routeEngine/Planner/Cache/Collision/Compaction/Fingerprint/
                  Invalidation/Repair/SegmentIndex + routing* + graph/index/worker
  geometry/    -> geometry, snapping
  selection/   -> selection, nodeLookup
  viewport/    -> viewport
  useEdgeRoutes.ts types.ts index.ts
```
硬约束继续保持：
```text
src/flow 不 import features
src/flow 不知道 WorkflowAction / AQL / NodeInspector
```

## 9. AQL Editor 本轮不动
`src/features/aql-editor` 已经有 `language/` 与 `view/`，结构合理。本轮不要顺手改：
```text
AQL protocol；Monaco provider；Language Service；WASM；Query Planner
```
否则“目录整理”会变成高风险架构 PR。

## 10. 后端：精准整理，不做全仓库搬家
建议基本不动：
```text
argusflow-browser/src/cdp/
argusflow-windows/src/uia/
argusflow-windows/src/window/
argusflow-windows/src/input/
argusflow-runtime/src/builtin_nodes/
argusflow-runtime/src/value_runtime/
src-tauri/src/commands/
```
这些已经是不错的“按能力域拆模块”样板。

### 真正值得整理：`argusflow-runtime/src`
建议：
```text
crates/argusflow-runtime/src/
  execution/  -> engine, scheduler, dispatcher, node_execution,
                 execution_events, run_context, run_inputs
  validation/ -> validator, validation_graph, validation_references
  component/  -> component_expander, component_registry, component_rewrite
  resource/   -> resource_table, resource_cleanup
  command/    -> command, command_job
  builtin_nodes/
  value_runtime/
  application.rs browser.rs node_registry.rs error.rs lib.rs
```
这里只做模块归位，不改变 Runtime/Planner/PreparedPlan 语义。

## 11. `argusflow-core` 只做轻整理
Core 本来就是领域契约集合。看到 `automation.rs / query.rs / resource.rs / workflow.rs / execution.rs` 并不等于必须全部变文件夹。

只在：
```text
文件已经包含多个稳定子域
或
明显接近/超过仓库 500 行约束
```
时拆。未来可自然演进成 `query/{model,capability}`、`workflow/{definition,node}`、`resource/{definition,reference}`，但不是本轮 P0。

后端优先级：
```text
P0 不动 browser/windows 边界
P1 整理 argusflow-runtime
P2 观察 argusflow-core
```
`src-tauri` 当前 `commands + lib/main/runtime` 也健康，不要为了“统一”套 Application/Infrastructure/Adapter 空层。

## 12. 自动迁移：前端首选 Node.js + TypeScript Compiler API
如果要“快速 + 精准”，前端最适合 Node.js，不是 C#：
```text
项目已依赖 TypeScript；不新增工具链；可解析真实 import/export；
移动前解析绝对目标；移动后重新计算相对路径；比 regex 安全。
```

建议新增：
```text
scripts/refactor-layout.mjs
scripts/refactor-layout.json
```
Manifest：
```json
{"moves":[
  {"from":"src/components/workflow/ActionNodeFields.tsx",
   "to":"src/components/workflow/inspector/node-fields/ActionNodeFields.tsx"}
]}
```
脚本算法：
```text
1. 读取 tsconfig，建立 TypeScript Program。
2. 收集 TS/TSX import/export source。
3. 移动前把相对引用解析为绝对目标。
4. 应用显式 MOVE_MAP。
5. 根据 source/target 新位置重新计算相对路径。
6. alias import 保持不动。
7. 更新指定 barrel index.ts。
8. 验证全部相对引用仍能落到真实文件。
9. 默认打印 dry-run report；只有 --apply 才写盘。
```
CLI：
```powershell
node scripts/refactor-layout.mjs --dry-run
node scripts/refactor-layout.mjs --apply
```
不要全仓库字符串 replace，不要用正则猜 import，不要在移动文件时顺便重命名领域类型。

## 13. Rust 自动迁移：Python 只做显式映射
可增加：
```text
scripts/refactor-rust-layout.py
```
它只负责：
```text
显式 MOVE_MAP；显式 crate::old -> crate::new；
显式 mod.rs/lib.rs 修改；dry-run；目标存在性检查；旧路径残留扫描
```
例如：
```python
PATH_REWRITES = {
    "crate::validation_graph": "crate::validation::graph",
    "crate::validation_references": "crate::validation::references",
}
```
不要让 Python 正则“理解 Rust 语义”。最终验证仍交给维护者执行 `rust-analyzer / cargo fmt --check / cargo check / cargo test`。

不推荐 C#：现有 Node/TS + Rust/Python 已覆盖需求，引入 dotnet 只增加环境成本。

## 14. 最安全实施顺序
### PR 1：UI primitives
新增 `Button / IconButton / SplitButton / Dialog / ConfirmDialog / DropdownMenu / Tooltip / FormField`。第一批迁移 `EditorPrimaryActions / EditorToolbarControls / WorkspaceEditorHeader`。目标：视觉不变、交互不变、业务不变，只去重。

### PR 2：`components/workflow`
只移动文件和修 import，不改 JSX/行为。

### PR 3：`features/workflow`
只做领域目录归位。

### PR 4：`flow`
单独整理 Canvas/Store/Routing/Interaction；`routing` 文件较多，最好独立 commit。

### PR 5：`argusflow-runtime`
只做 Rust module layout，不改执行算法、Planner、并发模型、Serde contract。

## 15. 重构后的硬规则
建议补进 `AGENTS.md`：
```text
1. 业务组件原则上不新增裸 <button>；基础交互从 components/ui 导入。
2. h-[26px]/text-[12px]/focus ring/disabled 等基础控件样式不得在业务层成组复制。
3. Dialog/Menu/Tooltip 只能由 components/ui 提供。
4. components/ui 禁止 import features/flow/workflow。
5. Node 属性组件统一进入 workflow/inspector/node-fields。
6. workflow 根目录只保留边界入口，不长期堆实现文件。
7. flow 按 canvas/store/routing/interaction 等内核职责分层。
8. 后端只在真实职责边界形成后建目录，不为了“整齐”套空层。
```

业务层仍可用 `className` 做布局差异：
```tsx
<Button variant="secondary" size="compact"
  className="hidden min-[1100px]:flex">
  检查流程
</Button>
```
响应式显示属于业务布局；26px 高度、边框、focus、disabled 属于 UI primitive。

## 16. Barrel export 要克制
建议只有明确边界提供公共入口：
```text
components/ui/index.ts
features/workflow/index.ts
flow/index.ts
```
规则：
```text
跨模块 -> 走 public index
模块内部 -> 直接相对 import
```
不要 `export * from './everything'`，否则循环依赖和边界泄漏会重新出现。

## 17. 本轮明确不做
```text
不改 Workflow JSON schema
不改 Tauri command 名称
不改 Rust serde 字段
不改 AQL 语法/Planner
不换 Zustand
不引大型 UI Design System
不把 Tailwind 重写成 CSS Modules
不建“大一统 shared/utils”
不把所有文件套到 5 层目录
不在移动文件 PR 中顺手重构业务算法
```
这次只有两个目标：**重复实现下降；职责在目录上可见。**

## 18. 验收标准
前端：
```text
[ ] components/ui 是唯一基础 UI 层
[ ] EditorPrimaryActions 不再定义私有 SplitActionButton
[ ] Toolbar/Dock 不再复制 IconButton 基础样式
[ ] Node 属性文件集中在 inspector/node-fields
[ ] components/workflow 根目录不再平铺几十个实现文件
[ ] features/workflow 按领域能力分组
[ ] flow 的 canvas/store/routing/interaction 一眼可区分
[ ] components/ui 不依赖 features/flow/workflow
```
后端：
```text
[ ] browser/windows 不做无意义搬家
[ ] runtime execution/validation/resource/component 职责清晰
[ ] src-tauri 继续保持薄
[ ] 不改变 serde contract
[ ] 不改变 Planner/PreparedPlan 语义
```
工程：
```text
[ ] 所有 move 可由 manifest 重放
[ ] 自动脚本默认 --dry-run
[ ] TS 脚本能检查断裂相对 import
[ ] Rust rewrite 只使用显式映射
[ ] 每个 PR 可独立回滚
```

## 19. 最终目标骨架
```text
src/
  components/
    ui/{button,form,overlay,layout,monaco}
    workflow/{workspace,inspector,palette,canvas,execution,overview}
    shell/
  features/
    aql-editor/
    workflow/{model,api,studio,nodes,values,components,execution,inputs}
  flow/{canvas,interaction,store,routing,geometry,selection,viewport}
```
```text
crates/
  argusflow-core/
  argusflow-runtime/src/{execution,validation,component,resource,command,builtin_nodes,value_runtime}
  argusflow-agent/
  argusflow-browser/
  argusflow-windows/
  argusflow-vision/
  argusflow-query/
  argusflow-query-wasm/
```

## 20. 建议落点
ArgusFlow 现在并不缺架构原则；现有文档和 `AGENTS.md` 已明确强类型、高内聚低耦合、UI/Feature/Flow、Core/Runtime/Platform、入口只装配、Tailwind 优先等约束。

真正缺的是：**把已经存在于语义和文档里的边界，落实到物理目录与基础 UI 组件。**

一句话：
```text
不改协议、不改算法、不换技术栈；
先把 UI primitive 做实，
再让目录结构真实表达职责。
```
优先顺序：
```text
UI primitives -> components/workflow -> features/workflow -> flow -> argusflow-runtime
```
如果只做第一轮，我建议：
```text
Button / IconButton / SplitButton / Dialog
+ components/workflow 归位
+ Node 属性统一进 inspector/node-fields
```
这是收益最高、风险最低的一刀。
