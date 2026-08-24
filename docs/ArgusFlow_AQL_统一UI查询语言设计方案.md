# ArgusFlow AQL 统一 UI 查询语言设计方案

> **AQL = Argus Query Language**
>
> 面向 ArgusFlow 的跨 UIA / CDP / AX / Vision 的统一 UI 查询语言。
>
> 本方案不追求 CSS 兼容，也不设计“残缺版 CSS Selector”。AQL 是一门独立的、面向 UI 语义与查询计划的 DSL。

---

## 1. 背景与目标

ArgusFlow 当前已经存在多种自动化后端：

- Windows UI Automation（UIA）
- Chrome DevTools Protocol（CDP）
- Accessibility Tree / AX Tree
- Vision / OCR
- Coordinate / SendInput

当前 `argusflow-core` 中的 `Selector` 本质上仍然是“后端选择器联合”：

```rust
pub enum Selector {
    Native {
        name: Option<String>,
        automation_id: Option<String>,
        control_type: Option<String>,
    },
    Browser {
        css: String,
    },
    VisualText {
        text: String,
        exact: bool,
    },
    Coordinate {
        x: i32,
        y: i32,
    },
}
```

这意味着当前的调用模型更接近：

```text
用户选择后端
    ↓
再为该后端编写 selector
```

而 ArgusFlow 更长期、更合理的模型应该是：

```text
用户描述“我要找什么 UI 元素”
            ↓
           AQL
            ↓
      Parser / AST / IR
            ↓
        Query Planner
      ┌─────┼──────┐
      ↓     ↓      ↓
     UIA    CDP   Vision
      ↓     ↓      ↓
        Resolved Target
            ↓
           Action
```

核心目标：

> **用户表达 UI 语义，而不是表达某个后端的实现细节。**

---

# 2. 为什么不应该做“类 CSS”的 ASL

最初可以很自然地想到：

```text
button[name="保存"] > text
```

或者：

```text
button#submit[name =~ /保存|Save/i]:enabled
```

这种语法对熟悉 CSS 的开发者非常直观。

但是它有一个严重的产品与工程问题：

> **一旦语言看起来像 CSS，用户就会自动认为它应该具备完整 CSS Selector 的语义。**

用户会自然尝试：

```css
button:not(:disabled)
div:has(> button)
:nth-child(2n + 1)
:is(...)
:where(...)
[data-x^="foo" i]
.foo.bar
#x > :first-child
```

如果 AQL 只支持其中一部分，就会形成一种非常糟糕的使用体验：

```text
“看起来是 CSS”
     ↓
“但很多 CSS 语法不支持”
     ↓
“而且部分语义在 UIA / CDP 下还不完全一致”
     ↓
“最终变成一个残缺版 CSS”
```

这会产生非常高的“错误知识迁移成本”。

因此本方案明确：

> **AQL 不是 CSS subset。**
>
> **AQL 不承诺任何 CSS compatibility。**
>
> **CSS 只作为 CDP 后端的显式原生 escape hatch。**

---

# 3. AQL 的核心定位

AQL 不应该被理解成：

```text
Argus Selector Language
```

更准确的定位应该是：

```text
Argus Query Language
```

因为 ArgusFlow 要做的不是单纯“选 DOM 元素”，而是在多个 UI 世界中执行统一查询：

```text
Windows UIA Tree
Chromium AX Tree
DOM Tree
Vision / OCR Tree
```

所以 AQL 的核心心智模型是：

```text
UI Query
```

而不是：

```text
CSS Selector
```

---

# 4. 语言哲学

AQL 建议明确遵守以下原则。

## 4.1 不追求 CSS 兼容

AQL：

- 不是 CSS subset
- 不是 XPath subset
- 不是 UIA condition 的文本包装
- 不是 CDP selector 的别名

它是一门独立 DSL。

---

## 4.2 语法表达语义，而不是 backend implementation

用户应该写：

```text
button(name = "保存")
```

而不是：

```text
ControlType == UIA_ButtonControlTypeId
```

也不是：

```css
button[aria-label="保存"]
```

AQL 描述的是：

```text
SemanticRole::Button
AccessibleName == "保存"
```

然后由 backend compiler 决定如何执行。

---

## 4.3 Backend-specific 能力必须显式

AQL 的 portable query：

```text
button(name = "保存")
```

而 backend-specific query 必须明确写出来，例如：

```text
button(
    uia.automation_id = "saveButton"
)
```

或：

```text
button(
    dom.test_id = "save-button"
)
```

以及完整原生 CSS：

```text
css("#app > div:nth-child(2) button")
```

用户一眼就能知道：

```text
portable?
还是 backend-specific?
```

---

## 4.4 无法完全支持时必须显式暴露

不应该为了“看起来统一”而偷偷降级。

例如某个操作在 UIA 下只能通过 TreeWalker 模拟：

```text
SupportLevel::Emulated
```

必须允许：

- explain
- analyzer
- validator
- UI inspector

明确展示。

不能表现得像原生能力。

---

## 4.5 AQL 本身不应该 Regex 化

Regex 非常有价值，但应该只是一个 predicate：

```text
name matches /保存|Save/i
```

而不是让整门语言变成：

```text
btn[name~=/(保存|Save)/i]:enabled >> txt
```

语言应该优先可读、可维护、可调试。

---

# 5. 推荐语法

## 5.1 最简单查询

```text
button(name = "保存")
```

含义：

```text
role = Button
AND
name = "保存"
```

---

## 5.2 多条件

```text
button(
    name = "保存",
    enabled = true
)
```

逗号默认表示：

```text
AND
```

---

## 5.3 contains

```text
button(
    name contains "保存"
)
```

---

## 5.4 starts_with

```text
text(
    name starts_with "订单"
)
```

---

## 5.5 ends_with

```text
text(
    name ends_with "完成"
)
```

---

## 5.6 Regex

```text
button(
    name matches /保存|Save/i
)
```

---

## 5.7 层级关系

### Descendant

```text
window(name contains "微信")
    >> button(name = "发送")
```

### Direct child

```text
dialog(name = "设置")
    > button(name = "确定")
```

这里虽然保留 `>` / `>>`，但整体语法已经明显不是 CSS。

---

# 6. 为什么不使用 `#id`

不建议：

```text
button#submit
```

因为前端用户会自然理解为：

```html
id="submit"
```

但 UIA 里更接近：

```text
AutomationId
```

这会制造错误语义映射。

建议：

```text
button(
    key = "submit"
)
```

AQL 将 `key` 定义为跨平台逻辑概念：

```rust
ElementKey
```

后端映射：

```text
UIA:
key → AutomationId

DOM:
key → id
```

这样用户不会误认为这是 CSS `#id`。

---

# 7. 为什么不使用 `.class`

不建议：

```text
button.primary
```

因为：

```text
DOM class
```

和：

```text
UIA ClassName
```

并不是同一个语义。

正确做法应该是显式 namespace：

```text
button(
    dom.class contains "primary"
)
```

或：

```text
button(
    uia.class_name = "Button"
)
```

这样不会制造虚假的 portable abstraction。

---

# 8. Portable AQL 与 Backend Escape Hatch

AQL 建议分为两个层次。

---

## 8.1 Portable AQL

默认绝大多数用户只使用 portable semantic properties：

```text
role
name
key
value
enabled
visible
focused
checked
selected
```

例如：

```text
button(name = "保存")
```

```text
textbox(
    name contains "用户名"
)
```

```text
window(name contains "微信")
    >> button(
        name matches /发送|Send/i
    )
```

这些应该尽量支持：

```text
UIA
CDP AX
Vision
```

---

## 8.2 Backend Escape Hatch

高级用户可以显式使用平台能力。

### UIA

```text
button(
    uia.automation_id = "btnSave"
)
```

```text
button(
    uia.class_name = "Button"
)
```

### DOM

```text
button(
    dom.test_id = "save-button"
)
```

### Raw CSS

```text
css("#editor > .toolbar button:nth-child(3)")
```

`css(...)` 的含义非常明确：

```text
这是 Browser / CDP 专用查询。
不保证跨平台。
```

未来如果需要也可以提供：

```text
uia(...)
```

用于原生 UIA 高级查询。

---

# 9. 推荐的 v1 运算符

AQL v1 不需要追求完整。

建议支持：

```text
=
!=
contains
starts_with
ends_with
matches
```

例如：

```text
name = "保存"

name != "取消"

name contains "保存"

name starts_with "订单"

name ends_with "完成"

name matches /保存|Save/i
```

相比：

```text
*=
^=
$=
~=
```

关键词更清晰，也不会给用户制造 CSS 心智。

---

# 10. 推荐的 Portable Properties

第一版建议控制在：

```text
name
key
value

enabled
visible
focused

checked
selected
```

以及 role 通过调用形式表达：

```text
button(...)
textbox(...)
window(...)
dialog(...)
```

内部映射为：

```rust
ElementRole
```

---

# 11. 推荐 ElementRole

```rust
pub enum ElementRole {
    Window,
    Dialog,
    Pane,

    Button,
    TextBox,
    CheckBox,
    Radio,
    ComboBox,

    List,
    ListItem,

    Tree,
    TreeItem,

    Tab,
    TabItem,

    Menu,
    MenuItem,

    Link,
    Image,

    Table,
    Row,
    Cell,

    Document,
    Text,
}
```

后续可以按真实 backend 映射能力增加。

不要一次把所有 ARIA role / UIA ControlType 全塞进去。

---

# 12. 强类型 AST

按照 ArgusFlow 当前“强类型优先”的工程规范，AQL 不应该被解析成：

```rust
HashMap<String, String>
```

也不应该使用：

```rust
Predicate {
    key: String,
    op: String,
    value: String,
}
```

建议直接建立强类型领域模型。

---

## 12.1 Query

```rust
pub struct UiQuery {
    pub expression: QueryExpr,
}
```

---

## 12.2 QueryExpr

```rust
pub enum QueryExpr {
    Match(ElementMatcher),

    Descendant {
        ancestor: Box<QueryExpr>,
        target: Box<QueryExpr>,
    },

    Child {
        parent: Box<QueryExpr>,
        target: Box<QueryExpr>,
    },

    Any(Vec<QueryExpr>),

    All(Vec<QueryExpr>),

    Not(Box<QueryExpr>),
}
```

---

## 12.3 ElementMatcher

```rust
pub struct ElementMatcher {
    pub role: Option<ElementRole>,
    pub predicates: Vec<PropertyPredicate>,
}
```

---

## 12.4 SelectorAttribute

```rust
pub enum SelectorAttribute {
    Name,
    Key,
    Value,

    Enabled,
    Visible,
    Focused,
    Checked,
    Selected,

    Uia(UiaAttribute),
    Dom(DomAttribute),
    Ax(AxAttribute),
}
```

---

## 12.5 MatchOperator

```rust
pub enum MatchOperator {
    Equal,
    NotEqual,
    Contains,
    StartsWith,
    EndsWith,
    Regex,
}
```

---

## 12.6 TreeRelation

如果不直接用 QueryExpr 表达，也可以抽象为：

```rust
pub enum TreeRelation {
    Child,
    Descendant,
    NextSibling,
    FollowingSibling,
}
```

但 v1 不建议过早支持全部 sibling 能力。

---

# 13. Query Compiler 架构

AQL 不应该只是：

```text
parse
↓
convert
```

而应该是一个小型编译器：

```text
Source
  ↓
Lexer
  ↓
Parser
  ↓
AST
  ↓
Normalizer
  ↓
Semantic Analyzer
  ↓
Capability Analyzer
  ↓
Query Planner
  ↓
Backend Compiler
  ↓
Execution Plan
```

---

# 14. Normalizer

例如：

```text
button(
    enabled = true,
    name = "保存"
)
```

和：

```text
button(
    name = "保存",
    enabled = true
)
```

应该 normalize 成同一个 IR。

Normalizer 可以负责：

- 规范 predicate 顺序
- 折叠 `all(...)`
- 处理常量布尔表达式
- 去除重复 predicate
- canonical string 输出
- query cache key

例如最终：

```text
button(enabled=true,name="保存")
```

可作为：

```text
normalized query key
```

---

# 15. UIA 编译模型

AQL：

```text
button(
    name matches /保存|Save/i,
    enabled = true
)
```

UIA 并不原生支持完整 Regex。

因此 UIA compiler 不应该失败，也不应该全量扫描整个树。

正确做法是拆成：

```text
Pushdown Predicate
+
Residual Predicate
```

---

## 15.1 Pushdown

UIA 可以原生过滤：

```text
ControlType = Button
IsEnabled = true
```

---

## 15.2 Cache

Regex 需要：

```text
Name
```

因此 compiler 自动加入：

```text
CacheRequest:
    Name
```

---

## 15.3 Residual Filter

Rust 本地执行：

```text
Regex(Name, /保存|Save/i)
```

---

## 15.4 最终 UIA Plan

```text
FindAllBuildCache
    Scope:
        Descendants

    Condition:
        AND(
            ControlType == Button,
            IsEnabled == true
        )

    Cache:
        Name

Residual:
    Regex(Name, /保存|Save/i)
```

这个设计与 ArgusFlow 原本强调的：

```text
减少跨进程 UIA 调用
使用 CacheRequest
避免逐属性 COM 调用
```

完全一致。

---

# 16. CDP 编译模型

同一个：

```text
button(
    name matches /保存|Save/i,
    enabled = true
)
```

CDP compiler 可以判断：

```text
这是 semantic query
```

优先考虑：

```text
Accessibility Tree
```

例如：

```text
AX role = button
```

然后执行 residual filter：

```text
name matches Regex
enabled = true
```

---

# 17. CSS 作为 CDP Fast Path

如果用户明确写：

```text
css("#login > input[type='password']")
```

那么不需要转成 AQL semantic matcher。

直接编译到：

```text
DOM.querySelector
DOM.querySelectorAll
```

这是显式 backend-native query。

---

# 18. CDP Query Planner

CDP compiler 可以逐步升级为：

```text
AQL
 ↓
Can CSS completely represent it?
 ├─ YES → DOM.querySelector(All)
 │
 └─ NO
     ↓
Can AX Tree narrow candidates?
 ├─ YES → Accessibility.queryAXTree
 │
 └─ NO
     ↓
Generic tree traversal

 ↓
Residual Filter
```

这样 CDP 不需要任何查询都走 Runtime.evaluate 或 JS。

---

# 19. SupportLevel

统一语言之后，不应该再只有：

```rust
supports() -> bool
```

建议引入：

```rust
pub enum SupportLevel {
    Native,
    Hybrid,
    Emulated,
    Unsupported,
}
```

定义：

### Native

Backend 原生可以完整表达。

### Hybrid

Backend 原生负责候选集缩小，然后 ArgusFlow residual filter。

### Emulated

需要 TreeWalker、JS traversal 或更高成本逻辑模拟。

### Unsupported

无法保证正确语义。

---

# 20. Query Capability

可以进一步定义：

```rust
pub struct QueryCapability {
    pub level: SupportLevel,
    pub estimated_cost: QueryCost,
    pub portability: Portability,
}
```

例如：

```text
button(name = "保存")
```

可能：

```text
UIA:
Native

CDP:
Native

Vision:
Hybrid
```

而：

```text
button(
    dom.test_id = "save-button"
)
```

则：

```text
UIA:
Unsupported

CDP:
Native
```

---

# 21. ActionRouter 必须升级

当前 ArgusFlow 的后端优先级大致类似：

```text
UIA
→ CDP
→ Visual
→ OCR
→ Grounding
→ SendInput
```

统一 AQL 后：

```text
button(name = "保存")
```

UIA 和 CDP 都可能声明支持。

如果仍然固定：

```text
UIA first
```

那么 Chromium 页面可能被 UIA 抢先处理。

这与 ArgusFlow 的性能架构目标冲突。

---

## 21.1 从 supports() 升级为 plan()

建议：

```rust
fn plan(
    &self,
    action: &AutomationAction,
    context: &ExecutionContext,
) -> QueryCapability;
```

或类似能力模型。

Backend 返回：

```text
能不能执行
执行质量
预计成本
当前上下文是否适合
```

---

# 22. Context-aware Routing

例如当前窗口是：

```text
Chrome
+
CDP session available
```

则：

```text
CDP:
Native
Cost = Low

UIA:
Hybrid / Native
Cost = Medium

→ CDP
```

原生 Win32：

```text
UIA:
Native
Cost = Low

CDP:
Unsupported

→ UIA
```

这比固定 fallback order 更合理。

---

# 23. Ambiguous Target

AQL 不应该默认“找第一个”。

例如：

```text
button(name = "确定")
```

找到：

```text
3 个
```

建议默认返回：

```text
AmbiguousTarget
```

而不是自动选择第一个。

明确规则：

```text
0 → TargetNotFound
1 → OK
>1 → AmbiguousTarget
```

---

# 24. 显式选择

未来可以支持：

```text
first(
    button(name = "确定")
)
```

或者：

```text
nth(
    button(name = "确定"),
    2
)
```

相比 CSS：

```text
:first-child
:nth-child(...)
```

AQL 语义更明确。

---

# 25. Query Algebra

AQL 可以逐步加入真正属于自己的查询代数。

## any

```text
any(
    button(key = "save"),
    button(name = "保存"),
    button(name matches /save/i)
)
```

---

## all

```text
all(
    button(),
    enabled(),
    visible()
)
```

具体语法最终可以进一步收敛。

---

## not

```text
not(
    button(name = "取消")
)
```

---

# 26. 为什么 `any(...)` 优于 CSS `:is(...)`

因为：

```text
any(...)
```

完全是查询语言心智。

不会引出：

```text
CSS specificity
selector list
pseudo-class compatibility
```

这些与 ArgusFlow 完全无关的概念。

---

# 27. explain 功能

强烈建议 AQL 从第一版就设计：

```text
EXPLAIN
```

或者 API：

```rust
explain_query(...)
```

输入：

```text
button(
    name matches /保存|Save/i,
    enabled = true
)
```

输出：

```text
AQL AST
────────────────────────

Role:
  Button

Predicates:
  Name Regex /保存|Save/i
  Enabled = true


UIA PLAN
────────────────────────

Scope:
  Descendants

Pushdown:
  ControlType = Button
  IsEnabled = true

Cache:
  Name

Residual:
  Regex(Name, /保存|Save/i)

Support:
  Hybrid


CDP PLAN
────────────────────────

Source:
  Accessibility Tree

Pushdown:
  role = button

Residual:
  Regex(name, /保存|Save/i)
  enabled = true

Support:
  Hybrid
```

---

# 28. Portability Analyzer

UI / Inspector 可以展示：

```text
UIA      ✓ Native
CDP      ✓ Hybrid
Vision   △ Hybrid
```

或者：

```text
UIA      ✕ Unsupported
CDP      ✓ Native
```

这对于 RPA workflow 稳定性非常重要。

---

# 29. 性能分析

AQL 的价值不仅是统一语法。

它同时应该承担：

```text
Query Optimization
```

例如：

```text
window(name contains "微信")
    >> button(name matches /发送|Send/i)
```

planner 可以做：

```text
先定位 Window
 ↓
缩小 subtree
 ↓
再查 Button
 ↓
最后执行 regex
```

而不是：

```text
Desktop 全树扫描
 ↓
全量 Regex
```

AQL 因此应该被理解成：

```text
UI Query + Query Planner
```

而不是：

```text
字符串 selector
```

---

# 30. Vision 未来接入

AQL 从第一天就不应该只为 UIA / CDP 设计。

例如：

```text
button(name = "保存")
```

未来 Vision backend 可以：

```text
OCR:
"保存"

+
GUI element detection:
Button

→ candidate
```

这样同一 AQL：

```text
button(name = "保存")
```

可以逐步支持：

```text
UIA
CDP AX
Vision
```

---

# 31. Semantic + Spatial Query

后期可以扩展：

```text
nearest(
    button(name = "编辑"),
    to = text(name = "张三")
)
```

或者：

```text
button(
    name = "提交",
    near = text(name = "订单号")
)
```

这种能力已经不是 CSS 可以很好表达的。

这进一步证明：

> AQL 不应该背 CSS 的历史包袱。

---

# 32. `has` 类能力的 AQL 表达

未来可以：

```text
row(
    has = text(name = "张三")
)
    >> button(name = "编辑")
```

而不是：

```css
row:has(text[name="张三"]) button
```

这样用户不会尝试把 CSS `:has()` 的全部规则套进来。

---

# 33. ArgusFlow Core 类型重构建议

当前：

```rust
pub enum Selector {
    Native { ... },
    Browser { ... },
    VisualText { ... },
    Coordinate { ... },
}
```

长期建议改为：

```rust
pub enum TargetLocator {
    Query(UiQuery),
    Visual(VisualQuery),
    Coordinate(ScreenPoint),
}
```

或者：

```rust
pub struct Target {
    pub locator: TargetLocator,
}
```

这样：

```text
UI Query
```

不再绑定 backend。

---

# 34. BackendPreference

如果确实需要用户或 planner 强制后端，可以设计：

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

注意：

```text
BackendPreference
```

只是 execution hint。

它不是 selector 的一部分。

同一个：

```text
button(name = "保存")
```

可以：

```text
Auto
UIA forced
CDP forced
```

Query 本身不变。

---

# 35. crate 拆分建议

ArgusFlow 当前已经有：

```text
argusflow-core
argusflow-runtime
argusflow-agent
argusflow-browser
argusflow-windows
argusflow-vision
```

建议新增独立 crate：

```text
argusflow-query
```

目录：

```text
crates/
├── argusflow-core/
│   └── src/
│       └── query.rs
│
├── argusflow-query/
│   └── src/
│       ├── lexer.rs
│       ├── parser.rs
│       ├── normalize.rs
│       ├── analyze.rs
│       ├── capability.rs
│       ├── error.rs
│       └── lib.rs
│
├── argusflow-windows/
│   └── src/
│       └── uia/
│           ├── compiler.rs
│           ├── plan.rs
│           ├── executor.rs
│           └── matcher.rs
│
└── argusflow-browser/
    └── src/
        └── cdp/
            ├── compiler.rs
            ├── plan.rs
            ├── executor.rs
            └── matcher.rs
```

---

# 36. crate 依赖方向

建议：

```text
argusflow-core
      ↑
argusflow-query
      ↑
 ┌────┴─────────┐
 │              │
windows       browser
```

不能：

```text
core
 ↓
UIA
```

也不能：

```text
core
 ↓
CDP
```

核心必须只知道：

```text
query semantics
```

而不知道具体平台。

---

# 37. Parser 错误设计

AQL 应该提供高质量 parser error。

例如：

```text
button(
    name ~= "保存"
)
```

应该返回：

```text
Unknown operator '~='

AQL does not use CSS attribute operators.

Did you mean:

    name matches /保存/

or:

    name contains "保存"
```

这一点非常重要。

因为它可以主动阻止用户把 CSS 知识迁移进 AQL。

---

# 38. 对 CSS 心智进行主动防御

如果用户写：

```text
button[name="保存"]
```

AQL parser 可以明确报：

```text
CSS-style attribute syntax is not valid AQL.

Use:

    button(name = "保存")

For raw browser CSS, use:

    css("button[name='保存']")
```

这是非常值得做的 UX。

不是“勉强兼容”。

而是：

> **主动告诉用户这不是 CSS。**

---

# 39. Canonical Formatter

建议提供：

```rust
format_query(...)
```

例如用户写：

```text
button(name="保存",enabled=true)
```

formatter 输出：

```text
button(
    name = "保存",
    enabled = true
)
```

这样：

- workflow diff 更稳定
- debugger 更容易读
- AI agent 更容易生成
- 用户习惯更统一

---

# 40. AQL v1 推荐语法范围

第一版建议只实现：

```text
Element Role

Properties:
    name
    key
    value
    enabled
    visible
    focused
    checked
    selected

Operators:
    =
    !=
    contains
    starts_with
    ends_with
    matches

Relations:
    >
    >>

Backend namespace:
    uia.*
    dom.*

Native escape:
    css(...)

Query combinator:
    any(...)
    not(...)

Selection:
    first(...)
    nth(...)
```

不要一开始实现：

```text
完整 sibling selector
复杂 arithmetic nth
CSS specificity
pseudo elements
shadow DOM 特殊语法
完整 XPath
```

这些都不是 v1 核心价值。

---

# 41. AQL v1 示例

## UIA / CDP Portable

```text
button(name = "保存")
```

---

## Multiple Predicates

```text
button(
    name = "保存",
    enabled = true,
    visible = true
)
```

---

## Regex

```text
textbox(
    name matches /用户|帐号|邮箱/i
)
```

---

## Descendant

```text
window(name contains "微信")
    >> button(name = "发送")
```

---

## Child

```text
dialog(name = "设置")
    > button(name = "确定")
```

---

## Explicit UIA

```text
button(
    uia.automation_id = "btnSave"
)
```

---

## Explicit DOM

```text
button(
    dom.test_id = "save-button"
)
```

---

## Raw CSS

```text
css("#editor > .toolbar button:nth-child(3)")
```

---

## Fallback Query

```text
any(
    button(key = "save"),
    button(name = "保存"),
    button(name matches /save/i)
)
```

---

## First

```text
first(
    button(name = "确定")
)
```

---

## nth

```text
nth(
    button(name = "确定"),
    2
)
```

---

# 42. Workflow 持久化建议

不建议 workflow 中只存原始字符串：

```json
{
  "query": "button(name = \"保存\")"
}
```

长期更稳妥的方案：

```text
用户编辑：
AQL string

保存 / 执行：
parse → normalized AST / structured contract
```

可以有两种设计。

---

## 42.1 方案 A：只保存字符串

优点：

- 简单
- 可读
- 手工编辑方便

缺点：

- parser version migration 复杂
- query semantic validation 依赖执行时解析

---

## 42.2 方案 B：保存结构化 IR

优点：

- 强类型
- schema migration 更清晰
- runtime 不需要重新解释语言细节

缺点：

- workflow JSON 更大
- 人工编辑不方便

---

## 42.3 推荐折中

建议保存：

```json
{
  "source": "button(name = \"保存\")",
  "language_version": 1
}
```

runtime：

```text
load
 ↓
parse
 ↓
normalize
 ↓
validate
 ↓
execute
```

后续如果性能或迁移需要，可以增加：

```text
compiled / normalized cache
```

但不要把 cache 作为持久化事实来源。

---

# 43. Language Version

建议第一天就有：

```rust
pub enum QueryLanguageVersion {
    V1,
}
```

workflow 中：

```json
{
  "language_version": 1,
  "source": "button(name = \"保存\")"
}
```

这样以后 AQL v2 改语义时不会破坏旧 workflow。

---

# 44. 与 ArgusFlow Workflow Schema 的关系

当前 ArgusFlow workflow 已经有：

```text
schema_version
```

AQL 语言版本不建议完全依附 workflow schema。

因为：

```text
Workflow Schema Version
```

和：

```text
Query Language Version
```

是两个独立协议。

建议分别演进。

---

# 45. Analyzer API

建议提供：

```rust
pub struct QueryAnalysis {
    pub normalized: UiQuery,
    pub portability: QueryPortability,
    pub capabilities: Vec<BackendQueryCapability>,
    pub warnings: Vec<QueryWarning>,
}
```

前端 Inspector 可以实时显示：

```text
Portable: Yes

UIA:
Native

CDP:
Hybrid

Vision:
Unsupported
```

---

# 46. QueryWarning

例如：

```text
BackendSpecificProperty
ExpensiveTraversal
RegexResidualFilter
AmbiguousRoleMapping
PotentialMultiMatch
UnsupportedBackend
```

这样 selector editor 不只是文本框，而是一个真正的 query IDE。

---

# 47. UI 设计建议

Action 节点中不要设计：

```text
Selector Type:
○ UIA
○ Browser
○ OCR
```

建议默认：

```text
Target Query

[AQL editor]
button(name = "保存")

────────────────

Portability

UIA    Native
CDP    Native
Vision Hybrid
```

高级模式：

```text
Backend details
```

才展示：

```text
Raw CSS
UIA-specific properties
```

---

# 48. Agent / AI 生成 AQL 的优势

AQL 比 CSS subset 更适合 AI Agent。

原因：

```text
button(
    name = "保存",
    enabled = true
)
```

拥有明确 grammar 和 semantic properties。

模型不容易混入：

```text
CSS specificity
:nth-child
HTML implementation details
```

可以给模型一个严格 grammar，让它生成：

```text
portable first
backend-specific only when necessary
```

这会极大提高自动生成 selector 的稳定性。

---

# 49. AQL 与 Computer Use 的长期结合

最终：

```text
Agent Intent

"点击张三这一行的编辑按钮"

        ↓

AQL Planner

row(
    has = text(name = "张三")
)
    >> button(name = "编辑")

        ↓

Backend Planner

UIA / CDP / Vision

        ↓

Resolved Element

        ↓

Invoke / Click
```

这已经不是传统 selector。

而是：

```text
UI Query Engine
```

---

# 50. 最终架构

```text
                    Workflow
                        │
                        ▼
                 AutomationAction
                        │
                        ▼
                      UiQuery
                        │
                        ▼
                     AQL Parser
                        │
                        ▼
                       AST
                        │
                        ▼
                    Normalizer
                        │
                        ▼
                 Semantic Analyzer
                        │
                        ▼
                Capability Analyzer
                        │
                        ▼
                   Query Planner
           ┌────────────┼────────────┐
           ▼            ▼            ▼
          UIA          CDP         Vision
        Compiler     Compiler      Compiler
           │            │            │
           ▼            ▼            ▼
        UIA Plan     CDP Plan     Vision Plan
           │            │            │
           └────────────┼────────────┘
                        ▼
                 Resolved Target
                        │
                        ▼
                     Action
```

---

# 51. 结论

ArgusFlow 不应该实现：

```text
“一个支持 UIA 的简易 CSS selector”
```

也不应该实现：

```text
CSS
 ↓
翻译成 UIA
```

最终应该实现的是：

# **AQL — Argus Query Language**

它的核心是：

```text
Portable Semantic Query
+
Backend-specific Escape Hatch
+
Query Compiler
+
Capability Analyzer
+
Query Planner
```

用户默认写：

```text
button(name = "保存")
```

而不是：

```css
button[name="保存"]
```

Regex 只是 predicate：

```text
name matches /保存|Save/i
```

Raw CSS 明确写：

```text
css(...)
```

UIA-specific 能力明确写：

```text
uia.*
```

DOM-specific 能力明确写：

```text
dom.*
```

这样既不会制造“残缺 CSS”的认知问题，也不会牺牲高级用户对原生后端能力的访问。

最终 ArgusFlow 的 selector 能力不再只是：

```text
统一 UIA / CDP selector
```

而会升级成：

> **跨 UI Tree 的统一查询语言与查询规划器。**

这会成为 ArgusFlow 架构中非常核心的一层。

---

# 52. 推荐实施顺序

建议按下面顺序实现。

## Phase 1

```text
argusflow-core
    UiQuery
    QueryExpr
    ElementRole
    PropertyPredicate
    MatchOperator
```

---

## Phase 2

新增：

```text
argusflow-query
```

实现：

```text
Lexer
Parser
Formatter
Normalizer
Error diagnostics
```

---

## Phase 3

实现：

```text
Capability Analyzer
SupportLevel
QueryAnalysis
```

此时还不需要真实 UIA / CDP 通信。

---

## Phase 4

实现：

```text
UIA Query Compiler
```

重点：

```text
Predicate Pushdown
CacheRequest
Residual Filter
```

---

## Phase 5

实现：

```text
CDP Query Compiler
```

重点：

```text
DOM fast path
AX semantic path
Residual Filter
```

---

## Phase 6

升级：

```text
ActionRouter
```

从：

```text
supports() -> bool
```

升级为：

```text
capability / plan / cost
```

---

## Phase 7

前端 Action Node 接入：

```text
AQL editor
Query validation
Explain
Portability
Backend capability
```

---

## Phase 8

扩展：

```text
Vision
nearest
has
spatial relation
query fallback
AI-generated AQL
```

---

# 53. 推荐第一版不要做的事情

为了控制复杂度，AQL v1 建议明确不做：

- CSS compatibility
- 完整 XPath
- CSS specificity
- CSS pseudo-class 兼容
- `nth-child(2n+1)` 之类的 arithmetic selector
- 完整 sibling algebra
- Shadow DOM 特殊 selector 语法
- 自动把 CSS 翻译成 UIA
- UIA 与 DOM property 的“强行同名统一”
- 默认 first-match
- 静态 UIA-first backend routing

---

# 54. 一句话定义

> **AQL 是 ArgusFlow 面向 UIA、CDP、Accessibility Tree 与 Vision 的平台无关 UI 查询语言。它表达元素语义与关系，由 Query Planner 根据运行环境编译为不同后端的最优执行计划。**
