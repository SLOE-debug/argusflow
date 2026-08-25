# ArgusFlow AQL 审计与重构方案

> 本文基于当前 ArgusFlow AQL 实现、最近两次 AQL 相关提交以及《ArgusFlow AQL 统一 UI 查询语言设计方案》进行工程审计。
>
> 本次重构不推翻现有 AQL 方向，而是在已有 Lexer / Parser / AST / Normalizer / Formatter / Analyzer / UIA Plan / CDP Plan 基础上，补齐语言工具链、运行时 Planner 和产品层之间的架构边界。
>
> **特别约束：AQL Editor 必须自研。**
>
> 不引入 Monaco Editor、CodeMirror 等完整编辑器框架；AQL 的文本模型、输入控制、渲染、语法高亮、诊断、格式化、补全、Hover、Code Action 等能力由 ArgusFlow 自己实现。

---

# 1. 审计范围

本次主要审计：

```text
2540d96
feat: 实现 AQL 统一 UI 查询语言

5890e10
feat: 完善 Action 节点与 AQL 编辑器
```

涉及模块主要包括：

```text
argusflow-core
    query.rs

argusflow-query
    lexer.rs
    parser.rs
    normalize.rs
    formatter.rs
    analyze.rs
    capability.rs
    error.rs

argusflow-windows
    uia/compiler.rs
    uia/plan.rs

argusflow-browser
    cdp/compiler.rs
    cdp/plan.rs

argusflow-agent
    ActionRouter

前端
    AqlEditor
    useAqlInspection
    ActionNodeFields
```

---

# 2. 审计结论

当前 AQL 大方向正确，不需要重新设计语言。

已经值得保留的部分：

```text
AQL 独立于 CSS / XPath
版本化 AqlQuery
强类型 UiQuery / QueryExpr
portable property
backend-specific namespace
UIA / DOM escape hatch
Normalizer
Canonical Query
UIA / CDP Logical Plan
SupportLevel
QueryCost
BackendPreference 与 Query 分离
AmbiguousTarget 语义
```

真正需要重构的是四个边界：

```text
AQL Source
    ↓
Language Tooling

AQL Semantics
    ↓
Backend Planner

Execution Context
    ↓
Prepared Plan

Prepared Plan
    ↓
UI Explain
```

最终原则：

> **UI 负责表达意图和展示结果。**
>
> **Language Service 负责理解 AQL。**
>
> **Planner 负责决定如何执行。**
>
> **Backend Compiler 是后端能力事实来源。**
>
> **Prepared Plan 是一次实际执行的事实来源。**

---

# 3. 不允许 UI 主导 Planner

这是本次重构最重要的架构约束之一。

当前 UI 可以提供：

```text
查找规则
AQL Source
高级执行约束
```

但 UI 不允许自行：

```text
判断哪个 Backend 更好
计算 SupportLevel
计算 QueryCost
决定 UIA / CDP / Vision 顺序
根据 capability 卡片改变 fallback
自行把 portable query 改写成 backend query
```

错误模型：

```text
UI
 │
 ├─ 看见 UIA Native
 │
 └─ 决定使用 UIA
        ↓
     Runtime
```

正确模型：

```text
UI
 │
 │ AutomationAction
 │ AQL
 │ Optional Backend Constraint
 ▼
Runtime Planner
 │
 │ ExecutionContext
 │ Backend Capability
 │ Runtime Availability
 │ Estimated Cost
 ▼
PreparedPlan
 │
 ├───────────────► UI Explain
 │
 ▼
Execute
```

UI 展示 Planner 的决定。

UI 不参与 Planner 的决定。

---

# 4. BackendPreference 的定位

`BackendPreference` 可以保留，但必须明确：

> 它是用户施加的执行约束或高级偏好，不是 UI Planner。

例如：

```rust
pub enum BackendPreference {
    Auto,
    WindowsUia,
    BrowserCdp,
}
```

默认：

```text
Auto
```

Auto 模式：

```text
Planner 自己选择最优 Backend
```

显式选择：

```text
WindowsUia
```

含义应该是：

```text
只允许或强烈约束 UIA 候选
```

而不是：

```text
UI 已经替 Runtime 做出了 Query Plan
```

默认产品 UI 中应隐藏该选项。

只在：

```text
高级设置
调试模式
Backend Explain
```

中暴露。

---

# 5. 当前 P0 问题：Capability Analyzer 使用全局 Feature Bag

当前 Analyzer 会把整棵查询树压缩成类似：

```rust
QueryFeatures {
    specific_backends,
    has_css,
    has_regex,
    has_any,
    has_not,
    has_relation,
    ...
}
```

这种模型无法正确表达 Query Algebra。

典型例子：

```text
any(
    button(
        uia.automation_id = "save"
    ),
    button(
        dom.test_id = "save"
    )
)
```

正确语义应该是：

```text
UIA:
    branch 1 Native
    branch 2 Unsupported
    => Query Supported

CDP:
    branch 1 Unsupported
    branch 2 Native
    => Query Supported
```

如果把整个查询压缩为：

```text
specific_backends = {
    WindowsUia,
    BrowserCdp
}
```

那么两个 Backend 都可能因为查询包含“另一个 Backend 的属性”而被判定 Unsupported。

这是 Query Algebra 无法被全局布尔 Feature 表达的问题。

## 5.1 重构原则

Capability 必须递归计算：

```text
Capability(Match)
Capability(Child)
Capability(Descendant)
Capability(Any)
Capability(Not)
Capability(First)
Capability(Nth)
```

例如：

```text
Capability(Any)
    =
选择当前 Backend 下
至少一个可以保持语义的 Branch
```

而不是：

```text
整棵树里出现了什么 feature
```

---

# 6. 当前 P0 问题：Analyzer 与 Compiler 出现双事实来源

目前：

```text
analyze.rs
```

维护：

```text
Backend 能否支持
Native / Hybrid / Emulated
Cost
```

与此同时：

```text
uia/compiler.rs
cdp/compiler.rs
```

又维护：

```text
哪些 Predicate 能 Pushdown
哪些需要 Residual
哪些完全 Unsupported
```

这已经形成两套 Backend 能力知识。

例如：

```text
name != "取消"
```

Analyzer 和 UIA Compiler 对它是否属于 native pushdown 已经存在产生不同结论的空间。

继续增加：

```text
has
nearest
spatial
AX
Vision
TreeWalker
Runtime capability
```

后，两套规则一定持续漂移。

## 6.1 最终原则

> **Backend Compiler 是能力事实来源。**

不要：

```text
Analyzer
    ↓
猜 Compiler 能做什么
```

应该：

```text
Backend Compiler
       ↓
    QueryPlan
       ↓
Plan Summary
       ↓
Capability
Cost
Warnings
Explain
```

即：

```rust
let plan = backend.compile(query, context)?;

let capability = plan.summary().capability;
let cost = plan.summary().estimated_cost;
```

而不是重新写一套 analyzer rules。

---

# 7. PreparedPlan：Planner 和 Executor 必须共享同一份计划

当前 Backend 接口类似：

```rust
fn plan(
    &self,
    action: &AutomationAction,
) -> ActionCapability;

async fn execute(
    &self,
    action: &AutomationAction,
) -> Result<...>;
```

这存在一个长期风险：

```text
Planner
  parse
  analyze
  calculate

        ↓

Executor
  再次 parse
  再次 compile
  再次判断
```

Planner 展示出来的计划和最终真正执行的计划可能不同。

## 7.1 推荐接口

重构为：

```rust
pub trait ActionBackend {
    fn prepare(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<PreparedCandidate, PlanRejection>;
}
```

候选：

```rust
pub struct PreparedCandidate {
    pub backend: BackendKind,
    pub capability: SupportLevel,
    pub estimated_cost: QueryCost,
    pub availability: RuntimeAvailability,
    pub plan: BackendExecutionPlan,
    pub explain: PlanExplain,
}
```

Router：

```text
Action
  ↓
prepare all candidates
  ↓
filter unsupported
  ↓
filter unavailable
  ↓
rank
  ↓
PreparedPlan
```

执行：

```text
PreparedPlan.execute()
```

而不是重新把原始 Action 交回 Backend 解析一次。

---

# 8. ExecutionContext 必须进入 Planner

Planner 不应该只知道：

```text
query
backend kind
```

还必须知道当前运行环境。

例如：

```rust
pub struct ExecutionContext {
    pub foreground_window: Option<WindowContext>,
    pub active_process: Option<ProcessContext>,
    pub browser_session: Option<BrowserSessionContext>,
    pub accessibility_state: AccessibilityContext,
    pub visual_cache: VisualCacheContext,
}
```

例如当前：

```text
Chrome
+
CDP Session Ready
```

那么：

```text
CDP
Native
Low Cost
Available

UIA
Native
Medium Cost
Available
```

Planner 应优先 CDP。

而当前：

```text
Win32 Notepad
```

则：

```text
UIA
Native
Low Cost

CDP
Unavailable
```

选择 UIA。

因此：

> **Query Capability 不是单纯的语法属性。**

最终 Plan 是：

```text
Query Semantics
+
Backend Compiler
+
Execution Context
+
Runtime Availability
```

共同产生的。

---

# 9. Semantic Support 与 Runtime Availability 必须拆开

当前 UI 已经可以展示：

```text
UIA
Native
Low Cost
```

但某些 Backend 的真实 Executor 仍可能尚未实现。

这会让用户理解成：

```text
UIA 当前已经可以执行
```

而实际含义只是：

```text
从语义模型来看
UIA 理论上可以原生表达
```

因此必须拆成两个维度。

```rust
pub enum SupportLevel {
    Native,
    Hybrid,
    Emulated,
    Unsupported,
}

pub enum RuntimeAvailability {
    Ready,
    MissingContext,
    Unavailable,
    NotImplemented,
}
```

UI Explain 才能准确表达：

```text
UIA

查询支持
    直接支持

当前状态
    执行器尚未接入
```

---

# 10. AQL Language Architecture 重构

现有：

```text
Lexer
 ↓
Parser
 ↓
UiQuery
 ↓
Normalizer
 ↓
Formatter
```

适合 Runtime。

但不足以支持真正的 AQL Editor。

新的语言架构建议：

```text
                         ┌─ Syntax Highlight
                         │
                         ├─ Diagnostics
                         │
AQL Source               ├─ Completion
    │                    │
    ▼                    ├─ Hover
Lossless Lexer           │
    │                    ├─ Code Action
    ▼                    │
Recovery CST ────────────┤
    │                    │
    ├──── Formatter      └─ Editor Decorations
    │
    ▼
Typed HIR / UiQuery
    │
    ▼
Semantic Validation
    │
    ▼
Normalize
    │
    ▼
Canonical IR
```

然后：

```text
HIR
 │
 ├──── UIA Compiler
 ├──── CDP Compiler
 └──── Vision Compiler
```

---

# 11. CST 与 HIR 必须分离

现有：

```rust
UiQuery
QueryExpr
ElementMatcher
PropertyPredicate
```

应该继续保留。

它们属于：

```text
HIR
High-level Intermediate Representation
```

也就是：

```text
已经理解语义
已经完成类型检查
```

的查询表示。

但 Editor 需要另一套：

```text
CST
Concrete Syntax Tree
```

CST 必须保留：

```text
Token
Whitespace
Source Range
Incomplete Node
Error Node
Delimiter
Trivia
```

例如用户只写：

```text
button(
    name =
```

Editor 仍然必须知道：

```text
button     Role
(          Delimiter
name       Property
=          Operator
```

而不是：

```text
Parser Error
整棵树不存在
```

---

# 12. Parser 从 Fail-fast 升级为 Error Recovery

Runtime Parser 可以继续提供：

```rust
parse_query(source)
    -> Result<UiQuery, AqlError>
```

但 Language Service 应新增：

```rust
parse_document(source)
    -> ParsedDocument
```

例如：

```rust
pub struct ParsedDocument {
    pub syntax: SyntaxTree,
    pub diagnostics: Vec<AqlDiagnostic>,
}
```

即使存在错误：

```text
button(
    name = ,
    enabled = true
)
```

Parser 仍然恢复到：

```text
button
name
enabled
true
```

使后面的：

```text
高亮
补全
Hover
第二个诊断
```

继续工作。

---

# 13. Formatter 必须与 Normalizer 分离

当前格式化实际上包含了：

```text
normalize
+
pretty print
```

这意味着用户点击：

```text
格式化
```

可能同时发生：

```text
Predicate 排序
Predicate 去重
Any Flatten
Any Deduplicate
Double Not 消除
```

这些属于 Semantic Normalization。

不是排版。

以后必须拆成：

```rust
format_source(...)
canonicalize_query(...)
normalize_query(...)
```

职责：

## format_source

只处理：

```text
空格
缩进
换行
括号
逗号
relation layout
```

不改变用户表达结构。

## normalize_query

处理：

```text
语义等价变换
Predicate 排序
去重
Any flatten
```

## canonicalize_query

产生：

```text
稳定 Cache Key
稳定 Query Identity
```

不要再让：

```text
Format
```

隐式执行 Semantic Rewrite。

---

# 14. 自研 AQL Editor 总体原则

AQL Editor 必须由 ArgusFlow 自研。

不采用：

```text
Monaco Editor
CodeMirror
Ace
```

作为编辑器内核。

但自研不意味着自己重写浏览器输入法。

核心原则：

> **自己实现语言编辑器。**
>
> **复用浏览器原生 Text Input / Selection / IME 能力。**

第一版明确不使用：

```text
contenteditable
```

作为核心输入模型。

原因：

```text
IME 行为复杂
Selection 不稳定
DOM normalization 不可控
Undo / Redo 难控制
浏览器差异较多
```

---

# 15. 自研 Editor 架构

建议：

```text
┌───────────────────────────────────────┐
│ AqlEditor                             │
│                                       │
│ ┌───────────────────────────────────┐ │
│ │ Gutter                            │ │
│ │ line / diagnostics               │ │
│ └───────────────────────────────────┘ │
│                                       │
│ ┌───────────────────────────────────┐ │
│ │ HighlightLayer <pre>              │ │
│ │                                   │ │
│ │ Role / Property / Operator        │ │
│ │ String / Regex / Namespace        │ │
│ └───────────────────────────────────┘ │
│                                       │
│ ┌───────────────────────────────────┐ │
│ │ DecorationLayer                   │ │
│ │                                   │ │
│ │ diagnostic underline              │ │
│ │ active bracket                    │ │
│ │ semantic hint                     │ │
│ └───────────────────────────────────┘ │
│                                       │
│ ┌───────────────────────────────────┐ │
│ │ InputLayer <textarea>             │ │
│ │                                   │ │
│ │ keyboard / IME / selection        │ │
│ │ clipboard / caret                 │ │
│ └───────────────────────────────────┘ │
│                                       │
│ Completion / Hover / Action Popup     │
└───────────────────────────────────────┘
```

Textarea 负责：

```text
IME
中文输入
光标
Selection
剪贴板
系统快捷键基础能力
```

ArgusFlow 自己负责：

```text
Document Model
Command
Undo / Redo
Highlight
Diagnostics
Completion
Hover
Formatting
Bracket Matching
Indent
Code Action
Semantic Decoration
```

---

# 16. Editor 内部模块

建议目录：

```text
src/features/aql-editor/

├── core/
│   ├── AqlDocument.ts
│   ├── TextEdit.ts
│   ├── Selection.ts
│   ├── LineIndex.ts
│   ├── Command.ts
│   ├── History.ts
│   └── EditorState.ts
│
├── language/
│   ├── LanguageClient.ts
│   ├── SyntaxToken.ts
│   ├── Diagnostic.ts
│   ├── Completion.ts
│   ├── Hover.ts
│   └── CodeAction.ts
│
├── view/
│   ├── AqlEditor.tsx
│   ├── HighlightLayer.tsx
│   ├── DecorationLayer.tsx
│   ├── InputLayer.tsx
│   ├── CompletionPopup.tsx
│   ├── HoverPopup.tsx
│   └── DiagnosticPopup.tsx
│
└── commands/
    ├── indent.ts
    ├── format.ts
    ├── completion.ts
    ├── bracket.ts
    └── navigation.ts
```

---

# 17. 不在 TypeScript 重写第二套 AQL Parser

自研 Editor 不应该变成：

```text
Rust Parser
+
TypeScript Parser
```

两套 Grammar。

否则：

```text
Runtime 接受
Editor 报错

或

Editor 接受
Runtime 报错
```

迟早发生。

因此语言事实来源仍然是：

```text
argusflow-query
```

建议增加：

```text
argusflow-query-wasm
```

结构：

```text
argusflow-query
    Lexer
    CST Parser
    HIR Lowering
    Formatter
    Semantic Diagnostics

         ↓

argusflow-query-wasm

         ↓

AQL Editor
```

浏览器 WebView 中直接运行同一套 Rust 语言引擎。

---

# 18. 为什么 Editor Language Service 使用 WASM

现在实时 AQL 分析需要：

```text
220ms debounce
    ↓
Tauri IPC
    ↓
Rust
```

这个模型适合：

```text
Runtime Plan Explain
```

但不适合：

```text
Syntax Highlight
Bracket Matching
Completion
Cursor Hover
Format on type
```

正确拆分：

```text
每次输入
   ↓
WASM Language Service
   ↓
Lexer / CST
   ↓
1~几 ms
   ↓
Highlight
Diagnostics
Completion
```

然后：

```text
输入稳定 100~250ms
   ↓
Runtime Planner
   ↓
ExecutionContext
   ↓
Backend Prepared Plans
   ↓
Explain
```

即：

```text
Language Feedback
```

和：

```text
Runtime Planning
```

完全分离。

---

# 19. Source Range 必须统一 UTF-16 协议

Rust 当前适合内部使用：

```text
UTF-8 Byte Offset
```

但浏览器：

```text
selectionStart
selectionEnd
String.slice
```

使用 UTF-16 Code Unit。

例如：

```text
button(name = "保存😀")
```

如果直接将 Rust Byte Offset 用于 DOM Selection：

```text
诊断范围
高亮范围
Code Action
```

都会错位。

因此：

```text
Rust Parser Internal
    UTF-8 ByteRange
```

协议边界转换成：

```rust
pub struct EditorRange {
    pub start: EditorPosition,
    pub end: EditorPosition,
}

pub struct EditorPosition {
    pub line: u32,
    pub utf16_column: u32,
}
```

前端统一使用：

```text
Line + UTF16 Column
```

禁止直接把 Rust Byte Offset 暴露给 Editor。

---

# 20. Syntax Highlight 设计

第一版至少支持：

```text
Role

button
textbox
window
dialog


Function

any
not
first
nth
css


Property

name
key
value
enabled
visible
focused
checked
selected


Namespace

uia
dom


Operator

=
!=
contains
starts_with
ends_with
matches


Literal

String
Boolean
Regex
Integer


Relation

>
>>


Punctuation

(
)
,
.
```

颜色由 Semantic Token Kind 决定。

禁止 UI 自己靠 Regex 猜 Token。

---

# 21. Semantic Decoration

AQL Editor 不应只做普通语法染色。

可以进一步提供：

```text
Backend-specific
Performance
Portability
Runtime Support
```

语义装饰。

例如：

```text
button(
    uia.automation_id = "save",
    name matches /保存|Save/i
)
```

可以表现：

```text
uia.*
    Backend-specific 标识

matches
    Residual filter 提示
```

Hover：

```text
uia.automation_id

Windows UIA
    Native

CDP
    Unsupported

Vision
    Unsupported
```

但数据来源必须是：

```text
Planner / Compiler Explain
```

而不是 Editor 自己判断。

---

# 22. Diagnostics 重构

当前：

```rust
QueryWarning {
    kind,
    backend,
    message: String,
}
```

会导致：

```text
Rust Domain
```

直接控制：

```text
产品中文文案
```

建议改成：

```rust
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub range: Option<SourceRange>,
    pub backend: Option<QueryBackend>,
    pub params: DiagnosticParams,
}
```

例如：

```text
code:
    aql.regex_residual_filter

backend:
    windows_uia
```

UI 再翻译成：

普通模式：

```text
这个条件需要额外筛选，执行可能稍慢
```

专家模式：

```text
UIA Residual Filter
Support: Hybrid
Cost: Medium
```

这样：

```text
Language / Runtime
```

不再绑定：

```text
UI Presentation
```

---

# 23. Explain 必须来自真实 PreparedPlan

目标：

```text
用户点击运行
```

前看到：

```text
本次将通过 CDP 执行
```

那么真正执行的也必须是这一份 CDP Plan。

禁止：

```text
Explain
    Analyze 一次

Runtime
    再 Plan 一次
```

推荐：

```text
Planner
   ↓
PreparedPlan
   ├──── explain()
   │
   └──── execute()
```

同一实例产生：

```text
UI Explain
```

和：

```text
Execution
```

---

# 24. Plan Explain 模型

建议：

```rust
pub struct PlanExplain {
    pub selected_backend: BackendKind,
    pub support: SupportLevel,
    pub cost: QueryCost,
    pub availability: RuntimeAvailability,
    pub portability: QueryPortability,
    pub steps: Vec<PlanStepExplain>,
    pub diagnostics: Vec<Diagnostic>,
}
```

UI 默认只展示：

```text
自动选择

Windows UI
直接支持
```

专家模式展开后：

```text
UIA Plan

Scope
    Descendants

Pushdown
    ControlType = Button
    IsEnabled = true

Cache
    Name

Residual
    Regex(Name)

Estimated Cost
    Medium
```

---

# 25. UI 文案重新定义

默认 UI 不暴露 Compiler 术语。

当前：

```text
执行动作
填写内容
定位方式
AQL 语义查询
后端偏好
自动规划
目标查询
查询说明
跨后端语义
原生
混合
模拟
低成本
```

建议：

```text
执行动作
    ↓
操作

填写内容
    ↓
输入内容

定位方式
    ↓
查找目标

AQL 语义查询
    ↓
智能查找
    AQL 作为辅助标签

自动规划
    ↓
自动选择（推荐）

目标查询
    ↓
查找规则

查询说明
    ↓
兼容性与性能

查询语法与谓词类型有效
    ↓
查询可用
```

Backend 技术细节进入：

```text
高级
开发者信息
Explain
```

---

# 26. Planner Ranking

Planner 推荐按：

```text
User Constraint
      ↓
Semantic Support
      ↓
Runtime Availability
      ↓
Execution Context Fitness
      ↓
Estimated Cost
      ↓
Stable Tie Break
```

而不是：

```text
UIA
↓
CDP
↓
Vision
```

固定执行顺序。

Tie Break 可以保留：

```text
UIA
CDP
Vision
...
```

但只能在：

```text
Capability 相同
Context Fitness 相同
Cost 相同
```

时使用。

---

# 27. Fallback 原则

Fallback 必须保持严格。

可以继续 fallback：

```text
BackendUnavailable
Runtime context disappeared
Session detached
```

不能因为：

```text
TargetNotFound
AmbiguousTarget
Semantic mismatch
Action execution failed
```

偷偷切换 Backend。

否则：

```text
CDP 找不到
↓
偷偷 UIA 找一个类似元素
↓
误点击
```

这是非常危险的行为。

原则：

> **Fallback 解决执行环境不可用。**
>
> **Fallback 不允许掩盖真实语义失败。**

---

# 28. Query Plan Cache

后续可以引入：

```text
Canonical Query
+
Language Version
+
Backend
+
Relevant Execution Context Fingerprint
```

形成：

```text
PlanCacheKey
```

例如：

```rust
pub struct PlanCacheKey {
    language_version: QueryLanguageVersion,
    canonical_query: QueryFingerprint,
    backend: BackendKind,
    context_kind: ContextFingerprint,
}
```

但：

```text
Compiled Plan
```

永远不是 Workflow 持久化事实来源。

Workflow 仍保存：

```json
{
  "language_version": 1,
  "source": "button(name = \"保存\")"
}
```

---

# 29. 推荐 crate 结构

保留：

```text
argusflow-core
```

负责：

```text
AQL semantic contract
UiQuery
QueryExpr
ElementRole
SelectorAttribute
```

扩展：

```text
argusflow-query

src/
├── syntax/
│   ├── token.rs
│   ├── lexer.rs
│   ├── parser.rs
│   ├── cst.rs
│   ├── recovery.rs
│   └── text.rs
│
├── hir/
│   ├── lower.rs
│   └── validate.rs
│
├── format/
│   ├── pretty.rs
│   └── canonical.rs
│
├── semantic/
│   ├── normalize.rs
│   └── diagnostic.rs
│
├── protocol/
│   ├── range.rs
│   ├── token.rs
│   ├── completion.rs
│   └── hover.rs
│
└── lib.rs
```

新增：

```text
argusflow-query-wasm
```

只负责：

```text
Rust Language Service
        ↓
WASM Boundary
        ↓
WebView
```

Backend Compiler 继续放：

```text
argusflow-windows
argusflow-browser
argusflow-vision
```

---

# 30. 测试体系

AQL 不应该只靠 Example Test。

建议增加几个层级。

## Syntax

```text
Lexer snapshot
Parser snapshot
Recovery parser
Unicode
Invalid syntax
CSS misuse diagnostic
```

## Formatter

必须保证：

```text
format(format(source)) == format(source)
```

并且：

```text
parse(format(source))
```

保持原语义。

## Canonical

必须保证：

```text
semantic equivalent query
        ↓
same canonical key
```

## Unicode

必须覆盖：

```text
中文
Emoji
Surrogate Pair
Regex Unicode
多行文本
```

验证：

```text
Rust UTF-8 Span
    ↔
Editor UTF-16 Position
```

## Planner

重点覆盖：

```text
any(UIA-specific, DOM-specific)

Chrome + CDP Ready

Chrome + CDP Missing

Win32 + UIA

Backend Preference constraint

Unavailable fallback

Semantic failure no fallback
```

## Editor

至少测试：

```text
中文 IME Composition
Selection
Paste
Undo
Redo
Multiline
Auto Indent
Bracket Pair
Completion Apply
Diagnostic Range
Format TextEdit
Emoji Cursor Position
```

---

# 31. 实施 Phase

## Phase 1：Language Foundation

目标：

```text
Lossless Token
Recovery CST
HIR Lowering
UTF-16 Editor Range
```

完成：

```text
syntax/*
protocol/range
parse_document
```

保持原：

```text
parse_query()
```

兼容 Runtime。

验收：

```text
错误源码仍能完整染色
一个文档可以返回多个 Diagnostic
中文 / Emoji Range 正确
```

---

## Phase 2：Formatter 分层

拆分：

```text
format_source
normalize_query
canonicalize_query
```

验收：

```text
Format 不再偷偷修改 Predicate 顺序和 Query Algebra
Canonical 结果保持稳定
旧 Workflow 仍兼容
```

---

## Phase 3：Planner 重构

完成：

```text
ExecutionContext
PreparedCandidate
PreparedPlan
RuntimeAvailability
PlanExplain
```

删除 Backend 能力双事实来源。

Capability 从真实 Compiler Plan 推导。

验收：

```text
UI Explain Plan
===
真正 Execution Plan
```

并正确处理：

```text
any(UIA, DOM)
```

---

## Phase 4：自研 AQL Editor Core

实现：

```text
Document Model
Input Layer
Highlight Layer
Decoration Layer
History
Command
Selection
LineIndex
```

接入：

```text
argusflow-query-wasm
```

第一阶段功能：

```text
Syntax Highlight
Diagnostic
Bracket Matching
Auto Indent
Format
```

---

## Phase 5：AQL IDE 能力

增加：

```text
Completion
Hover
Code Action
Semantic Decoration
Quick Fix
```

例如：

```text
button[name="保存"]
```

提供 Quick Fix：

```text
转换为：

button(name = "保存")
```

---

## Phase 6：产品层重构

默认用户只看到：

```text
操作
输入内容
查找目标
查找规则
查询可用
```

高级模式：

```text
AQL
Backend Constraint
Plan Explain
Canonical Query
Backend Capability
```

---

## Phase 7：Executor 接入

依次完成：

```text
UIA PreparedPlan Executor
CDP PreparedPlan Executor
```

执行器直接消费：

```text
PreparedPlan
```

禁止重新解析原始 AQL。

---

# 32. 本轮建议优先级

## P0

必须先解决：

```text
Capability Feature Bag 无法正确表达 Query Algebra
Analyzer / Compiler 双事实来源
Planner 缺少 ExecutionContext
Planner / Executor 不是同一个 PreparedPlan
Semantic Support 与 Runtime Availability 混淆
```

## P1

紧接着解决：

```text
Lossless CST
Recovery Parser
Formatter / Normalizer 分离
UTF-16 Editor Range
Diagnostic Code 与 UI 文案分离
自研 AQL Editor Core
```

## P2

后续增强：

```text
Completion
Hover
Code Action
Semantic Decoration
Plan Cache
Plan Trace
Vision Query Compiler
Spatial Query
```

---

# 33. 明确禁止的实现方式

本次重构明确不采用：

```text
Monaco / CodeMirror 作为 AQL Editor 内核

contenteditable 作为主要文本模型

TypeScript 再实现一套 AQL Parser

UI 计算 Backend Capability

UI 决定 Backend Routing

Analyzer 和 Compiler 各维护一份 Backend 规则

Formatter 隐式执行 Semantic Normalize

Runtime Execute 再次重新 Plan

Backend execution error 随意触发另一个 Backend fallback

默认 UI 暴露大量 Compiler 内部术语
```

---

# 34. 最终目标架构

```text
                    Workflow
                       │
                       ▼
                AutomationAction
                       │
                       ▼
                    AQL Source
                       │
          ┌────────────┴─────────────┐
          │                          │
          ▼                          ▼
   AQL Language Service         Runtime Planner
          │                          │
          │                     ExecutionContext
          │                          │
          ▼                          ▼
   Lossless Lexer               Prepare Candidates
          │                    ┌─────┼──────┐
          ▼                    ▼     ▼      ▼
   Recovery CST              UIA    CDP   Vision
          │                    │     │      │
          ▼                    ▼     ▼      ▼
     Typed HIR              Backend Compiler
          │                    │     │      │
          ├── Formatter         ▼     ▼      ▼
          ├── Diagnostics     Prepared Backend Plans
          ├── Completion              │
          ├── Hover                   ▼
          └── Editor Tokens       Planner Rank
                                      │
                                      ▼
                                 PreparedPlan
                                  ┌────┴─────┐
                                  ▼          ▼
                               Explain     Execute
                                  │
                                  ▼
                                  UI
```

这里最重要的依赖方向是：

```text
UI
 ↓
Language Service / Planner API
```

而绝不能：

```text
Planner
 ↓
依赖 UI 状态来决定执行逻辑
```

---

# 35. 一句话定义本次重构

> **本次 AQL 重构的目标不是增加更多 selector 语法，而是把 AQL 从“可解析的查询 DSL + 一个文本框”升级为 ArgusFlow 自己的 UI Query Compiler、Runtime Planner 和自研 Query IDE。**
>
> **用户只负责表达“我要找什么”，AQL Language Service 负责理解它，Backend Compiler 负责证明如何实现它，Runtime Planner 负责根据真实运行环境选择执行计划，而 UI 永远只负责输入意图与展示 Planner 的结果。**