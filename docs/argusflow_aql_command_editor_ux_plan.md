# ArgusFlow AQL 与命令脚本编辑器实施方案

## 1. 最终决策

编辑器统一采用 Monaco，但语言能力仍按领域边界拆分：

- Monaco 负责文本模型、光标与选区、undo/redo、括号、补全 UI、Hover UI、诊断装饰、快速修复入口和展开布局。
- AQL 继续以 `argusflow-query-wasm` 为唯一语言事实来源，通过自定义 Monaco language provider 提供语义能力。
- PowerShell 使用 Monaco 内置 `powershell` 语言定义。
- CMD 使用 Monaco 内置 `bat` 语言定义。
- 本次不接入 PowerShell Editor Services，也不实现 CMD language server。
- 不修改 `AqlQuery`、`CommandOperation`、`ValueExpr` 或工作流 JSON 契约。

这项决策取代此前的自研 textarea 编辑内核方案。业务层不再维护重复的 selection、history、bracket、overlay 和渲染同步逻辑。

## 2. 目标体验

### AQL

- 「查找规则」位于高级设置之前，是 query locator 的主要编辑区域。
- 内联编辑器默认高度为 `220px`，支持行号、水平滚动和展开编辑。
- 鼠标在 token 上停留 `300ms` 后，由 Monaco 显示 sticky Hover。
- 不提供单独的「说明」按钮，点击或移动光标不会主动弹出说明。
- `Ctrl+Space` 使用 Monaco 补全 UI。
- 诊断同时显示为 Monaco marker 和编辑器下方的产品摘要。
- 格式化通过标题栏按钮执行，并作为一次可撤销的 Monaco edit 写入文档。
- Rust/WASM code action 通过 Monaco light bulb / quick fix UI 提供。

### PowerShell / CMD

- 固定文本脚本使用 Monaco 多行编辑器，保留换行与长行。
- 编辑器显示行号与对应 shell 的基础语法着色。
- 非 literal 脚本继续显示 `workflow_input`、`node_output` 或 `variable` 引用字段。
- Direct runner 的程序、参数等字段继续使用单行控件。
- runner 切换只改变 Monaco language id 和 badge，不清空脚本。

## 3. 架构边界

```text
components/ui/monaco
├── MonacoEditor.tsx       通用受控编辑器与命令句柄
├── modelRegistry.ts       稳定 URI 模型和延迟释放
└── monacoLoader.ts        Monaco、Worker、shell 语言和主题按需加载

features/aql-editor/language
├── LanguageClient.ts      Rust/WASM 边界
├── MonacoAqlLanguage.ts   Monaco provider 适配
├── messages.ts            诊断与 Hover 产品文案
└── types.ts               AQL language service 契约

features/aql-editor/view
├── AqlEditor.tsx          AQL 编辑器编排
├── DiagnosticPopup.tsx    诊断摘要
└── PlanExplanation.tsx    Planner 只读反馈

components/workflow
├── ActionNodeFields.tsx   query locator 编排
├── CommandNodeFields.tsx  command runner 编排
├── CommandScriptField.tsx shell 脚本业务组合
└── ValueExprFields.tsx    值来源与 literal presentation
```

`src/components/ui` 不依赖 `features` 或 `flow`。AQL adapter 依赖通用 Monaco API 和 AQL 语言契约；workflow 组件只负责选择业务语言、构造模型 URI 并写回领域值。

## 4. AQL language provider

自定义语言 ID：

```text
argusflow-aql
```

WASM 能力映射：

| Rust/WASM 能力 | Monaco provider |
|---|---|
| `inspect().parsed.semantic_tokens` | `DocumentSemanticTokensProvider` |
| `inspect().parsed.diagnostics` | model markers |
| `completions()` | `CompletionItemProvider` |
| `hover()` | `HoverProvider` |
| `inspect().formatted_source` | `DocumentFormattingEditProvider` |
| `codeActions()` | `CodeActionProvider` |

协议位置仍使用零基 UTF-16 行列；适配器只在 Monaco 边界转换成一基行列，不复制 Rust AST 或 HIR。

Monarch tokenizer 只提供 WASM 尚未返回 semantic tokens 时的低成本基础着色。它不是第二套 AQL parser，不负责校验或业务决策。

## 5. 模型与状态

每个业务字段使用稳定 URI：

```text
inmemory://argusflow/workflow/{node-id}/locator-aql
inmemory://argusflow/workflow/{node-id}/command-script
```

通用模型注册表按 URI 引用计数。最后一个编辑器卸载后，模型延迟到当前事件循环末尾释放，因此从 Inspector 内联区切换到展开 Drawer 时，新编辑器会接管同一模型并保留 undo 栈。

Monaco 模型仍是受控输入：

```text
model change
  -> onChange(full source)
  -> AqlQuery / ValueExpr update
  -> workflow store
```

外部值确实变化时才同步回 Monaco，避免父组件每次渲染重置编辑历史。

## 6. Hover 行为

AQL 与 shell 编辑器均采用 Monaco / VS Code 对齐的配置：

```text
delay: 300ms
sticky: true
hidingDelay: 300ms
```

Hover 仅由指针停留或 Monaco 原生键盘命令触发。业务组件不监听 selection 来请求 Hover，也不维护自定义 Hover popup 或「说明」按钮。

## 7. Inspector 布局

`InspectorEditorSection` 是业务无关的结构化内容容器，负责：

- 标题与语言 badge；
- toolbar slot；
- footer slot；
- 内联与右侧展开 Drawer；
- Escape 与背景点击关闭 Drawer。

它不拥有文档值，也不感知 AQL、PowerShell、CMD 或工作流契约。

## 8. ValueExpr presentation

`ValueExprFields` 继续拥有值来源选择和引用字段。literal 展示采用判别联合：

```ts
type LiteralPresentation =
  | { type: 'single_line' }
  | { type: 'custom'; render: (props: LiteralEditorProps) => ReactNode };
```

默认仍是单行输入。`CommandScriptField` 仅在 shell literal 场景注入 Monaco renderer，因此不会让程序名、参数、变量等普通字段无条件变成大型编辑器。

## 9. 非目标

- AQL grammar、parser、HIR 或 Planner 重写。
- Runtime command 执行契约调整。
- 工作流 JSON 迁移或兼容分支。
- PowerShell LSP、shell completion 或脚本 formatter。
- CMD parser 或静态诊断。
- 将 AQL 语言事实复制到 TypeScript。

## 10. 验收标准

### AQL

- [x] 使用 Monaco 文本模型和编辑交互。
- [x] Hover 停留 `300ms` 后显示，不存在「说明」按钮。
- [x] AQL WASM 补全、Hover、诊断、semantic tokens、格式化和 code action 接入 Monaco。
- [x] 默认编辑区至少 `220px` 高。
- [x] 查找规则位于高级设置之前。
- [x] 支持展开编辑并复用同一文档模型。
- [x] Planner 语义与 Runtime 契约不变。

### Command

- [x] PowerShell literal 使用 `powershell` Monaco language。
- [x] CMD literal 使用 `bat` Monaco language。
- [x] 多行内容原样写回，不丢失换行。
- [x] 非 literal ValueExpr 保持引用配置。
- [x] Direct runner 单行字段不受影响。
- [x] 支持展开编辑并复用同一文档模型。
- [x] Command Runtime contract 不变。

## 11. 验证策略

- jsdom 不具备 Monaco 所需的 Worker 与布局环境，React 组件测试使用等价的受控 textarea mock 验证领域写回、语言选择和布局切换。
- AQL parser、formatter、completion、Hover 和 code action 的语言事实继续由 Rust 测试覆盖。
- 前端回归测试验证 AQL 格式化、无独立说明按钮、展开模式、脚本语言映射、多行 round-trip、非 literal 与 Direct runner 行为。
