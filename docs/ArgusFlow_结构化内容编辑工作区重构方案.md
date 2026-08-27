# ArgusFlow 结构化内容编辑工作区重构方案

> 审查基线：`SLOE-debug/argusflow` 当前 `main`
>
> 最新提交：`07c3b8d536f5b8900f8673d6d1aff17729f9ac14`
>
> 上一关键提交：`12c1fa320842b62f8fef84d5d91619ec45d1390e`
>
> 核心结论：**不要再把 Monaco 塞在右侧 312px 左右的 Inspector 中，再依赖固定宽度 Drawer 作为“真正编辑器”。**
>
> 右侧属性栏应该负责“看状态、看摘要、进入编辑”；真正的 AQL / PowerShell / CMD 编辑应该进入中央工作区的可调整 Dock。

---

## 1. 最新提交之后，问题已经从“编辑器能力”变成“产品空间模型”

最近两个提交已经把底层编辑能力补得很完整：

- `12c1fa3`：接入 Monaco、AQL Provider、PowerShell/CMD 多行编辑、稳定模型 URI、内联 + Drawer。
- `07c3b8d`：补 Shiki PowerShell/Batch TextMate Grammar、VS Code Light+、AQL semantic token scope、`fixedOverflowWidgets` 与 Hover 体验。

所以现在真正不舒服的不是 Monaco，而是：

```text
一个 IDE 级编辑器
    ↓
被塞进表单型 Inspector
    ↓
只给 220px 高
    ↓
想认真编辑必须打开 Drawer
```

这套空间模型本身不合理。

---

## 2. 当前实现的几个硬事实

### 2.1 Inspector 默认只有 312px

当前：

```ts
const INSPECTOR_PANEL_WIDTH = {
  default: 312,
  min: 272,
  max: 480,
} as const;
```

这对名称、ID、Select、超时等普通字段很合理。

但对：

```text
AQL
PowerShell
CMD
JSON
Template
Expression
```

不合理。

在 312px 里放 Monaco，会同时损失：

- 可读宽度；
- 行号/gutter；
- 长字符串阅读；
- Hover / completion 空间；
- 代码结构感。

结果就是：看起来“很专业”，实际可用空间却很少。

---

### 2.2 AQL / Script 内联区都固定 220px 高

AQL 当前：

```tsx
className={
  layout === 'expanded'
    ? 'h-[calc(100vh-220px)] min-h-[420px]'
    : 'h-[220px]'
}
```

PowerShell / CMD 当前：

```tsx
className={
  layout === 'expanded'
    ? 'h-[calc(100vh-170px)] min-h-[480px]'
    : 'h-[220px]'
}
```

这会自然形成：

```text
小 Monaco = 预览
Drawer = 真编辑器
```

既然第一层大多数时候不好用，就不应该启动完整 Monaco。

---

### 2.3 Drawer 的宽度确实写死在组件中

当前：

```tsx
w-[min(900px,calc(100vw-48px))]
```

内部又：

```text
max-w-[960px]
```

所以用户的感觉是准确的：

> 编辑空间不是工作区按需求分配，而是组件作者替用户决定了“最多大概 900px”。

这适合详情 Drawer，不适合长期写代码。

---

## 3. Drawer 是不是正确选择？

结论：**不是 AQL / Script 的默认主编辑形态。**

Drawer 适合：

- 临时详情；
- 少量表单；
- 创建/编辑记录；
- 次级设置。

AQL / Script 则属于：

```text
持续专注的文档编辑任务
```

用户可能会：

- 写 20 行 PowerShell；
- 看 Hover；
- 用补全；
- 修诊断；
- 格式化；
- 对照流程节点；
- 看运行日志；
- 再回来继续修改。

把这种任务设计成 `aria-modal=true` 的右侧 Drawer，会把 Canvas 变成背景，反而破坏 RPA Studio 最需要的“流程上下文”。

---

## 4. 产品原则：Inspector 负责属性，Workspace 负责内容

建议明确一条长期规则：

### Scalar Property

例如：

```text
name
timeout
path
backend
boolean
ID
reference
```

→ 右侧 Inspector 直接编辑。

### Structured Document

例如：

```text
AQL
PowerShell
CMD
JSON
Template
Expression
```

→ Inspector 只显示：

- 类型；
- 状态；
- 摘要；
- 小预览；
- 编辑入口。

真正编辑进入中央 Workspace。

---

## 5. Inspector 不再放 Monaco

### AQL 改成摘要卡

```text
查找规则                              AQL

┌──────────────────────────────────┐
│ ✓ 查询可用                       │
│                                  │
│ css(                             │
│   "#hotsearch-content-wrapper…"  │
│ )                                │
│                                  │
│ 浏览器 · 直接支持 · 低开销       │
│                         [编辑规则]│
└──────────────────────────────────┘
```

这里不是 editor，而是：

```text
AqlFieldSummary
```

点击整卡或“编辑规则”进入 Workspace Editor。

### Script 改成摘要卡

```text
脚本                         PowerShell

┌──────────────────────────────────┐
│ 固定文本 · 7 行                  │
│                                  │
│ [Console]::InputEncoding = …     │
│ [Console]::OutputEncoding = …    │
│ $desktop = …                     │
│                                  │
│                         [编辑脚本]│
└──────────────────────────────────┘
```

如果 ValueExpr 是：

```text
workflow_input
node_output
variable
```

则仍然直接在 Inspector 编辑引用字段，不打开代码工作区。

---

## 6. 真正编辑器：Workspace Editor Dock

当前中央区已经是：

```text
Canvas
────────
Console
```

建议升级为统一：

```text
Canvas
════════════════  可拖拽 splitter
Workspace Dock
```

Dock 承载：

```text
Structured Editor
任务
运行记录
日志
告警
```

不要再另外造第三套浮层体系。

---

## 7. 为什么推荐 Bottom Dock

代码最需要的是横向空间。

Bottom Dock 可以拿到中央工作区几乎完整宽度，同时保留：

- 左节点库；
- 上方 Canvas；
- 右 Inspector；
- 流程上下文。

用户可以同时看到：

```text
流程
+
代码
+
属性
```

比 modal Drawer 更符合 Studio / IDE。

---

## 8. Dock 必须可调尺寸

当前 `WorkflowWorkspace` 的 Console 本身也是固定 `304px`。

建议这次一起把中央区域做成真正的 split layout。

状态建议：

```ts
type WorkspaceDockMode =
  | { type: 'collapsed' }
  | { type: 'docked'; height: number }
  | { type: 'maximized' };
```

默认：

```text
clamp(320px, 38vh, 520px)
```

限制：

```text
min: 220px
max: workspace height * 0.75
```

用户直接拖动 splitter。

不再写死：

```text
304px
900px
```

---

## 9. Dock 三种状态

### collapsed

只保留底部 tab/header，约 38px。

### docked

Canvas 与 Editor 共存。

### maximized

Editor 占据中央工作区。

注意：

> maximized 不是全屏 modal。

仍然保留 Studio 外壳和流程上下文。

---

## 10. 不再叫“展开编辑”

当前“展开编辑”是 Drawer 心智。

新产品文案建议：

```text
编辑规则
编辑脚本
```

进入 Dock 后：

```text
最大化
还原
关闭编辑器
```

用户不需要理解 `inline / expanded`。

---

## 11. 推荐中央区域效果

正常：

```text
┌──────────────────────── Workflow Canvas ─────────────────────────┐
│                                                                  │
│   Browser  ──> Delay ──> Collect Links ──> PowerShell            │
│                                                                  │
├══════════════════════ draggable splitter ═════════════════════════┤
│ AQL · 批量获取热搜标题和链接                          _  □  ×    │
├───────────────────────────────────────────────────────────────────┤
│ 1  css(                                                          │
│ 2      "#hotsearch-content-wrapper a.title-content ..."          │
│ 3  )                                                             │
│                                                                  │
│                                                 ✓ 查询可用        │
└───────────────────────────────────────────────────────────────────┘
```

不遮罩，不 modal，不固定 900px。

---

## 12. `InspectorEditorSection` 应退出主编辑职责

当前它同时做：

```text
header
actions
footer
inline Monaco
Drawer
Portal
Escape
backdrop
```

职责太多。

建议拆为：

```text
StructuredFieldSummary
WorkspaceEditorDock
WorkspaceEditorHeader
WorkspaceDockResizeHandle
```

Drawer 不再由字段组件自己决定。

---

## 13. Workspace Editor 打开目标必须强类型

建议：

```ts
type StructuredEditorTarget =
  | {
      type: 'aql';
      nodeId: string;
    }
  | {
      type: 'command_script';
      nodeId: string;
    };
```

不要存 renderer callback，也不要 `any`。

Workspace Host 根据 `type + nodeId` 从 workflow state 解析：

- 当前 value；
- language；
- model URI；
- node label；
- onChange。

---

## 14. UI State 不写进 Workflow JSON

建议：

```ts
type WorkspaceEditorState = Readonly<{
  target: StructuredEditorTarget | null;
  mode: 'docked' | 'maximized';
  dockHeight: number;
}>;
```

这是纯界面状态。

可先放 `App`，再抽 `useWorkspaceEditor()`。

不进入 Rust / Runtime / Workflow schema。

---

## 15. Inspector 与 Workspace 的通信

当前：

```text
AqlEditor
  自己 setLayout('expanded')
```

建议：

```text
AqlFieldSummary
    ↓
onOpenEditor({ type: 'aql', nodeId })
    ↓
WorkspaceStructuredEditor
```

Script 同理：

```text
ScriptFieldSummary
    ↓
onOpenEditor({ type: 'command_script', nodeId })
```

“在哪里编辑”应该由 Workspace 决定，而不是字段组件自己决定。

---

## 16. Monaco 稳定 URI 继续保留

现有：

```text
inmemory://argusflow/workflow/{node-id}/locator-aql
inmemory://argusflow/workflow/{node-id}/command-script
```

是正确的。

新方案反而更简单：

```text
不再有
Inspector Monaco ↔ Drawer Monaco
```

只有 Workspace Monaco。

---

## 17. Model Registry 后续可以更像 Workbench

现在最后一个 editor 卸载后 `setTimeout(..., 0)` 就 dispose，主要是为了 inline → Drawer 同轮切换保 undo。

未来可以改为：

- 当前 workflow 会话内保留；
- 或 LRU 最近 8 个结构化文档。

这样关闭 AQL、打开 Script、再回 AQL，Undo 栈还能保留。

P0 不必做，P1 再做。

---

# 18. AQL“格式化按钮不能点”的根因

当前 `AqlEditor.tsx`：

```tsx
disabled={!formattedSource || formattedSource === query.source}
```

所以按钮灰掉有两类原因：

```text
1. Language Service 没有 formatted_source
2. formatted_source 与当前源码完全相同
```

你的默认 CSS 查询非常容易命中第二种。

---

## 19. 为什么默认 CSS 查询几乎必然“不能格式化”

默认 Workflow AQL：

```text
css("#hotsearch-content-wrapper a.title-content .title-content-title")
```

Rust Pretty Formatter 对 `QueryExpr::Css` 当前直接：

```rust
QueryExpr::Css { selector } =>
    format!("css({})", quote_string(selector))
```

也就是：

```text
selector 再长也永远输出一行
```

因此当前源码已经等于 formatter 结果：

```text
formattedSource === query.source
```

前端就把按钮禁用了。

所以这不完全是 Monaco 坏了，更准确是：

> **Formatter 的排版规则与产品需要的可读性不一致，再叠加“结果相等就禁用按钮”，最终看起来像格式化功能失效。**

---

## 20. Format 状态不应该只是一个灰按钮

建议：

```ts
type FormatAvailability =
  | { type: 'loading' }
  | { type: 'invalid' }
  | { type: 'clean' }
  | { type: 'dirty' };
```

### loading

按钮 disabled，Tooltip：

```text
AQL 语言服务正在启动
```

### invalid

disabled，Tooltip：

```text
请先修复语法错误
```

### clean

显示：

```text
✓ 已格式化
```

而不是一个原因不明的灰色“格式化”。

### dirty

正常：

```text
格式化
```

---

## 21. Format 按钮应该走 Monaco 标准 Format Document

当前已经注册：

```text
DocumentFormattingEditProvider
```

但标题按钮又自己：

```text
formattedSource
→ replaceAll()
```

这实际上有两条 formatting command path。

建议统一：

```text
Rust Formatter
    ↓
Monaco DocumentFormattingEditProvider
    ↓
editor.action.formatDocument
```

给 `MonacoEditorHandle` 增加：

```ts
formatDocument: () => Promise<void>;
```

内部：

```ts
await editor.getAction('editor.action.formatDocument')?.run();
```

Toolbar 和 `Shift+Alt+F` 最终走完全同一条链。

---

## 22. Rust Formatter 也应该修长 `css()`

建议：

```rust
const PRETTY_LINE_WIDTH: usize = 100;
```

短 CSS：

```text
css("#submit")
```

继续一行。

长 CSS：

```text
css(
    "#hotsearch-content-wrapper a.title-content .title-content-title"
)
```

只改 AQL 外层布局，不碰 selector 内容。

规则：

```text
if indent + rendered_length <= 100:
    单行
else:
    css(
        "..."
    )
```

这样默认长 selector 点击格式化时就会有可见变化。

---

# 23. Script 还有一个独立问题：默认模板本身仍是一行

当前默认 PowerShell 使用：

```ts
[
  ...
].join('; ')
```

也就是说：

> 即使已经换成 Monaco 多行编辑器，默认脚本文档依然只有一个物理行。

所以用户仍然会看到很长的一条脚本。

这不是 Monaco 问题，是 fixture/source 本身的问题。

---

## 24. 默认脚本应该改成真实换行

建议：

```ts
[
  '[Console]::InputEncoding = ...',
  '[Console]::OutputEncoding = ...',
  '$desktop = ...',
  '$path = ...',
  '$content = ...',
  '[IO.File]::WriteAllText(...)',
  '[Console]::Out.Write($path)',
].join('\n')
```

持久化源码统一 LF。

如果某个 Windows 执行边界要求 CRLF，在执行边界转换，不要把 `; ` 当 UI 换行。

---

# 25. Inspector AQL 摘要应该展示什么

推荐：

```text
AQL
✓ 查询可用

css("#hotsearch-content-wrapper …")

浏览器 · 直接支持 · 低开销

[编辑规则]
```

如果有错误：

```text
AQL
✕ 查询存在 2 个问题

第 1 行：缺少右括号

[修复规则]
```

不要启动 Monaco。

---

# 26. Script 摘要

Literal：

```text
PowerShell
固定文本 · 7 行

[Console]::InputEncoding = …
[Console]::OutputEncoding = …
$desktop = …

[编辑脚本]
```

Reference：

```text
PowerShell
来源：节点输出

节点：prepare_script_1
输出：script
```

Reference 直接在 Inspector 配，不需要打开 editor。

---

# 27. 这样还能减少不必要的 Monaco 实例

现在只要选中带 AQL / Script 的节点，就会启动 Monaco。

以后节点复杂后，这会带来：

- worker；
- model；
- provider；
- layout；
- ResizeObserver；
- Hover widget；
- tokenization。

Summary 用普通 React + `<code>`。

只有用户真的点“编辑”时才加载 Monaco，产品性能也更合理。

---

# 28. Hover 最新提交继续保留

`07c3b8d` 的：

```text
fixedOverflowWidgets: true
Shiki grammar
Light+ theme
```

都保留。

这些能力进入宽阔 Workspace 后会比 312px Inspector 中自然很多。

---

# 29. 推荐组件关系

```text
App
│
├── NodePalette
│
├── WorkflowWorkspace
│   │
│   ├── WorkflowCanvas
│   │
│   └── WorkspaceDock
│       │
│       ├── WorkspaceStructuredEditor
│       │   ├── AqlEditor
│       │   └── CommandScriptEditor
│       │
│       ├── Tasks
│       ├── Runs
│       ├── Logs
│       └── Alerts
│
└── NodeInspector
    │
    ├── normal fields
    ├── AqlFieldSummary
    └── ScriptFieldSummary
```

---

# 30. AQL Editor 自身应该变简单

新 `AqlEditor` 只负责：

- Monaco；
- AQL provider；
- format；
- diagnostics；
- status；
- planner explain。

不再负责：

```text
Portal
Drawer
inline/expanded layout
```

---

# 31. Command Script 也应该拆开

现在 `CommandScriptField` 同时处理：

```text
ValueExpr source
Inspector section
layout
Monaco
```

建议改为：

Inspector：

```text
CommandScriptSourceFields
+
ScriptFieldSummary
```

Workspace：

```text
CommandScriptEditor
```

---

# 32. “数据来源”仍留 Inspector

这是重要产品细节。

```text
数据来源 [固定文本 ▼]
```

仍在右侧。

只有 `literal` 时显示 Script Summary。

切换到：

```text
节点输出
```

则摘要卡消失，直接出现：

```text
生产节点 ID
输出端口
```

比现在 ValueExpr 卡里再嵌 220px Monaco 清晰得多。

---

# 33. Dock 与 Console 应统一

当前底部已经有：

```text
任务
运行记录
日志
告警
```

建议升级成：

```text
WorkspaceDockPanel
```

而不是再新增另一个独立 panel。

打开 AQL：

```text
[AQL · 批量获取热搜标题和链接 ×] [任务] [运行记录] [日志] [告警]
```

打开 Script：

```text
[PowerShell · 写入桌面百度热搜.txt ×] [任务] [运行记录] [日志] [告警]
```

v1 同时只需要一个 Structured Editor。

未来再考虑多文档 tabs。

---

# 34. 切换节点时不要自动关 Editor

workflow store 是实时受控写回，不存在传统“未保存文件”。

因此：

```text
编辑脚本
→ 点击另一个节点看 ID
→ 继续编辑脚本
```

应该成立。

Editor Header 必须显示：

```text
PowerShell · 写入桌面百度热搜.txt
write_baidu_news_1
```

并提供：

```text
定位节点
```

点击后选中并聚焦 Canvas 中对应节点。

这会建立很好的：

```text
代码 ↔ 流程
```

关系。

---

# 35. 文件级修改建议

## `src/components/ui/InspectorEditorSection.tsx`

不再作为主编辑容器。

建议删除或退出 AQL/Script 使用。

不要继续保留：

```text
inline + portal drawer
```

作为核心形态。

---

## `src/App.tsx`

增加：

```text
WorkspaceEditorState
dockHeight
mode
openStructuredEditor
closeStructuredEditor
```

---

## `src/components/workflow/WorkflowWorkspace.tsx`

把固定：

```text
304px
```

改成可拖拽 Workspace Dock。

---

## `src/components/workflow/WorkflowConsolePanel.tsx`

建议升级命名/职责：

```text
WorkspaceDockPanel
```

或者抽出 Utility Tabs。

不要把“编辑器”硬塞进仍叫 Console 的组件。

---

## `src/components/workflow/ActionNodeFields.tsx`

当前：

```tsx
<AqlEditor ... />
```

改：

```tsx
<AqlFieldSummary
  ...
  onEdit={() => openStructuredEditor({
    type: 'aql',
    nodeId,
  })}
/>
```

---

## `src/components/workflow/CommandScriptField.tsx`

不再直接 render Monaco。

改成：

```text
source config
+
ScriptFieldSummary
```

---

## 新增

```text
src/components/workflow/WorkspaceStructuredEditor.tsx
src/components/workflow/AqlFieldSummary.tsx
src/components/workflow/ScriptFieldSummary.tsx
src/components/workflow/WorkspaceDockResizeHandle.tsx
src/components/workflow/WorkspaceEditorHeader.tsx
```

---

# 36. AQL Format 修改

## `src/features/aql-editor/view/AqlEditor.tsx`

去掉：

```ts
disabled={!formattedSource || formattedSource === query.source}
```

改为明确 format state。

并把：

```text
replaceAll(formattedSource)
```

改为：

```text
formatDocument()
```

---

## `src/components/ui/monaco/MonacoEditor.tsx`

Handle 增加：

```ts
formatDocument: () => Promise<void>;
```

---

## `src/features/aql-editor/language/MonacoAqlLanguage.ts`

继续保留：

```text
DocumentFormattingEditProvider
```

作为前端唯一格式化入口。

---

## `crates/argusflow-query/src/formatter.rs`

增加长 `css()` 的 line-width pretty rule。

---

# 37. 默认 Script 修改

## `src/features/workflow/defaultWorkflowTemplate.ts`

当前：

```ts
].join('; ')
```

改：

```ts
].join('\n')
```

必须做。

否则大编辑器上线以后，默认示例仍然一整条横向脚本，第一印象继续很差。

---

# 38. P0：建议本轮一次完成

### Workspace

- [ ] 移除 Inspector 内 AQL Monaco。
- [ ] 移除 Inspector 内 literal Script Monaco。
- [ ] Inspector 改 Structured Summary。
- [ ] 新增 Workspace Editor Dock。
- [ ] Dock 默认 bottom。
- [ ] Dock 高度可拖拽。
- [ ] Dock 支持最大化 / 还原。
- [ ] 不用 backdrop。
- [ ] 不用 `aria-modal=true`。
- [ ] 不固定 900px。

### AQL

- [ ] Format 状态有明确文案。
- [ ] Toolbar 走 Monaco Format Document。
- [ ] 长 `css()` 能 pretty-wrap。
- [ ] 默认百度 AQL 格式化后有可见变化。

### Script

- [ ] 默认 PowerShell fixture 改真实多行。
- [ ] Literal 只在 Workspace 编辑。
- [ ] Reference ValueExpr 继续留 Inspector。

---

# 39. P1

- Editor Dock 高度记忆；
- 文档 cursor 记忆；
- model LRU / session 生命周期；
- “定位节点”；
- Header 显示 node label + ID；
- Script summary 显示真实行数；
- AQL planner explain 在 Dock 中折叠展示。

---

# 40. P2

- 多 Structured Document tabs；
- 用户自选 bottom/right dock；
- editor zoom；
- wrap toggle；
- PowerShell language server；
- workspace layout persistence。

本轮不要一次做大。

---

# 41. 测试重点

### Inspector

选中 AQL/Script 节点：

```text
只出现 summary
```

不创建 Monaco。

点击编辑后 Workspace 才出现 Monaco。

### Dock

- drag resize 有 min/max clamp；
- maximize → restore 不丢 value；
- model URI 稳定；
- 切换节点不自动关闭当前编辑文档。

### AQL Format

长 CSS 输入：

```text
css("#hotsearch-content-wrapper a.title-content .title-content-title")
```

应能转换为可读多行（必要时测试使用更长 selector 保证超过阈值）。

Toolbar 与 `Shift+Alt+F` 结果一致。

已格式化时显示：

```text
已格式化
```

而不是原因不明的灰色按钮。

### Default Script

断言：

```text
script.value.includes('\n')
```

并且默认 fixture 有多行。

---

# 42. 为什么不只是把 Drawer 做成可拖宽

把 900px Drawer 改成 resizable 只能解决：

```text
宽度写死
```

没有解决：

- 为什么代码是 modal？
- 为什么 Inspector 还要启动一个几乎没价值的小 Monaco？
- 为什么代码和 Canvas 不能自然同时工作？
- 为什么 Console 又是另一套固定 panel？
- 为什么结构化文档的打开方式由字段组件决定？

所以：

```text
Resizable Drawer
```

只是局部修补。

更正确的是：

```text
Structured Document
    ↓
Workspace
```

---

# 43. Drawer 还需要保留吗？

桌面端默认不需要。

极窄窗口可以直接：

```text
Dock → maximized
```

也比 Drawer 更自然。

如果未来某些普通详情页需要 Drawer，可以保留一个通用 Drawer primitive，但不要让 AQL / Script 依赖它。

---

# 44. 最终产品原则

以后看到字段先判断：

```text
这是“属性”还是“文档”？
```

属性：

```text
Inspector
```

文档：

```text
Inspector Summary
+
Workspace Editor
```

这条规则会让 ArgusFlow 后续扩展 JSON、模板、表达式、Agent Prompt 时都保持一致。

---

# 45. 最终建议

保留：

- Monaco；
- Shiki；
- AQL WASM Language Service；
- Monaco Provider；
- stable model URI；
- Hover / Completion / Diagnostics。

推翻的是：

```text
Inspector 220px Monaco
+
900px modal Drawer
```

这套空间模型。

改成：

```text
Inspector
  = 状态 + 摘要 + 编辑入口

Workspace
  = 可调整、可最大化、非模态的真正编辑器
```

同时修掉：

```text
AQL 长 css() 永远一行
格式化按钮无解释 disabled
默认 PowerShell 用 '; ' 拼成一行
```

完成以后，AQL 和 Script 才真正成为 ArgusFlow Studio 的一等编辑对象。

---

# 46. 推荐 PR 拆分

## PR 1

```text
refactor(workspace): replace inspector editor drawer with structured editor dock
```

包括：

- Summary Card；
- Workspace Dock；
- resize；
- maximize；
- AQL/Script workspace host；
- 移除 Inspector Monaco/Drawer。

## PR 2

```text
fix(aql): make format state explicit and wrap long css queries
```

包括：

- FormatAvailability；
- Monaco `formatDocument()`；
- provider 单一入口；
- Rust css pretty format；
- tests。

## PR 3

```text
fix(workflow): keep default PowerShell script as multiline source
```

包括：

- `join('\n')`；
- fixture tests；
- UI behavior tests。

---

# 47. 本次审查涉及文件

```text
src/App.tsx

src/components/ui/InspectorEditorSection.tsx
src/components/ui/monaco/MonacoEditor.tsx
src/components/ui/monaco/modelRegistry.ts

src/components/workflow/WorkflowWorkspace.tsx
src/components/workflow/WorkflowConsolePanel.tsx
src/components/workflow/ActionNodeFields.tsx
src/components/workflow/CommandScriptField.tsx

src/features/aql-editor/view/AqlEditor.tsx
src/features/aql-editor/language/MonacoAqlLanguage.ts
src/features/aql-editor/language/useLanguageDocument.ts

src/features/workflow/defaultWorkflowTemplate.ts

crates/argusflow-query/src/formatter.rs
crates/argusflow-query/src/language.rs

docs/argusflow_aql_command_editor_ux_plan.md
```

---

# 48. 目标画面

### Inspector

```text
查找目标
[语义查找（AQL）]

查找规则                                AQL
┌───────────────────────────────────────┐
│ ✓ 查询可用                            │
│ css("#hotsearch-content-wrapper …")   │
│ 浏览器 · 直接支持 · 低开销            │
│                              编辑规则 │
└───────────────────────────────────────┘

高级设置
```

### Workspace Dock

```text
┌───────────────────────────────────────────────────────┐
│ AQL · 批量获取热搜标题和链接    已格式化  最大化  ×  │
├───────────────────────────────────────────────────────┤
│ 1  css(                                               │
│ 2      "#hotsearch-content-wrapper ..."               │
│ 3  )                                                  │
└───────────────────────────────────────────────────────┘
```

### Script

```text
脚本来源
[固定文本]

脚本                              PowerShell · 7 行
┌───────────────────────────────────────┐
│ [Console]::InputEncoding = …          │
│ [Console]::OutputEncoding = …         │
│ $desktop = …                          │
│                              编辑脚本 │
└───────────────────────────────────────┘
```

这应该成为下一版的目标体验。
