# ArgusFlow AQL 中文化与视觉定位可读性改造方案

> 目标仓库：`SLOE-debug/argusflow`  
> 目标：让 AQL 可以使用中文进行编写与阅读，同时让使用者能够明确理解 **Vision/OCR 视觉层究竟在定位什么、依据什么定位、为什么命中/不命中**。  
> 本方案基于当前仓库的 AQL Parser / HIR / Formatter / Language Service / WASM / Monaco / UIA / CDP / Vision / Planner / Scene Inspector 实现进行设计。

---

## 0. 结论先行

### 推荐方案

**不要把 ArgusFlow 内部 AQL AST、UIA/CDP/Vision Compiler 全部“改成中文”。**

应该采用：

```text
中文 AQL / 英文 AQL
        │
        ▼
  双语 Surface Syntax
        │
        ▼
  同一个 UiQuery / QueryExpr
        │
        ├────────► UIA Compiler
        ├────────► CDP Compiler
        └────────► Vision Compiler
```

也就是：

> **中文只属于“源码语言层”，内部语义仍使用当前强类型英文 IR。**

推荐最终形态：

```text
用户输入
    ↓
文字(名称 = "发送")

Parser
    ↓
QueryExpr::Match {
    role: ElementRole::Text,
    predicates: [
        SelectorAttribute::Name = "发送"
    ]
}

Canonical
    ↓
text(name="发送")

Backend Compile
    ├─ UIA
    ├─ CDP
    └─ Vision/OCR
```

这有四个直接收益：

1. UIA / CDP / Vision 不需要各维护一套中文语义。
2. 英文 AQL 与中文 AQL 可以生成完全相同的 canonical query。
3. 已有缓存、规范化、参数解析、Planner 排序逻辑无需重写。
4. 中文 AQL 可以专注解决“可读性”，而视觉 Explain 专门解决“实际定位语义”。

---

# 1. 当前 AQL 架构分析

## 1.1 当前持久化与强类型语义已经天然适合做语言方言

当前核心模型位于：

```text
crates/argusflow-core/src/query.rs
```

主要结构为：

```rust
AqlQuery
    language_version
    source
    bindings

UiQuery
    expression: QueryExpr
```

其中：

```text
AqlQuery.source
```

是用户编辑的源码；

而：

```text
UiQuery / QueryExpr / ElementMatcher / SelectorAttribute
```

已经是与源码拼写无关的强类型语义。

因此中文化最合理的切入点不是修改 `QueryExpr`，而是：

```text
Source
 ↓
Lexer
 ↓
Parser
 ↓
UiQuery
```

中的 Source/Lexer/Parser 层。

---

## 1.2 当前执行链路

当前实际链路可以归纳成：

```text
AQL Source
    │
    ▼
argusflow-query
    │
    ├─ lossless lexer
    ├─ parser
    ├─ HIR / UiQuery
    ├─ normalize
    ├─ canonicalize
    ├─ formatter
    └─ language service
    │
    ▼
UiQuery / QueryExpr
    │
    ├───────────► Windows UIA Compiler
    │
    ├───────────► Browser CDP Compiler
    │
    └───────────► Vision Query Compiler
                        │
                        ▼
                    OCR Scene
```

Runtime Planner 再根据：

```text
语义支持能力
Runtime Availability
Execution Context
Cost
Backend Policy
```

选择实际执行后端。

因此：

> 中文 AQL 不应该改变 Planner 的判断，也不应该改变 Backend Compiler 的事实来源。

---

# 2. 当前实现中与中文化直接相关的关键点

## 2.1 当前 Lexer 实际不支持中文标识符

文件：

```text
crates/argusflow-query/src/syntax/lexer.rs
```

当前标识符规则本质为：

```rust
is_identifier_start:
    ASCII alphabetic
    _

is_identifier_continue:
    ASCII alphanumeric
    _
    .
```

因此下面的源码当前无法成为合法 Identifier：

```aql
文字(名称 = "发送")
```

它会在 lossless lexer 阶段直接成为错误 token。

所以中文化的 **第一个 P0 改动** 必须是 Unicode Identifier。

推荐不要简单使用：

```rust
char::is_alphabetic
```

而使用 Unicode XID：

```rust
unicode_ident::is_xid_start
unicode_ident::is_xid_continue
```

例如：

```rust
fn is_identifier_start(ch: char) -> bool {
    ch == '_' || unicode_ident::is_xid_start(ch)
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '.'
        || ch == '_'
        || unicode_ident::is_xid_continue(ch)
}
```

这样能够正确支持：

```aql
文字
名称
最近
锚点
目标
下方
$群名
```

同时不会破坏：

```aql
uia.automation_id
dom.test_id
```

---

# 3. 当前 AQL 的语法能力盘点

当前 Parser 支持的 Query Algebra 主要是：

```text
Match
Child
Descendant
Any
Not
First
Nth
Nearest
Css
```

对应源码：

```aql
button(name = "保存")

window(name contains "微信")
    >> button(name = "发送")

any(
    button(name = "保存"),
    button(name = "确定")
)

not(button(name = "取消"))

first(button(name = "保存"))

nth(button(name = "保存"), 2)

nearest(
    anchor = text(name = "网络结果"),
    target = text(name = "联系人"),
    direction = below,
    index = 1,
    metric = edge_gap
)

css("#toolbar > button")
```

---

# 4. 当前角色清单

当前角色：

| Canonical | 推荐中文 |
|---|---|
| `window` | `窗口` |
| `dialog` | `对话框` |
| `pane` | `面板` |
| `button` | `按钮` |
| `textbox` | `文本框` |
| `checkbox` | `复选框` |
| `radio` | `单选框` |
| `combobox` | `下拉框` |
| `list` | `列表` |
| `list_item` | `列表项` |
| `tree` | `树` |
| `tree_item` | `树节点` |
| `tab` | `选项卡` |
| `tab_item` | `选项卡项` |
| `menu` | `菜单` |
| `menu_item` | `菜单项` |
| `link` | `链接` |
| `image` | `图像` |
| `table` | `表格` |
| `row` | `行` |
| `cell` | `单元格` |
| `document` | `文档` |
| `text` | **`文字`** |

建议把 `text` 翻译成：

```text
文字
```

而不是：

```text
文本
```

原因是：

```text
textbox -> 文本框
text    -> 文字
```

区分度更高。

更重要的是：

> 当前 Vision/OCR 真正支持的就是 `text`，所以“文字”能够直接提醒用户这是一个视觉文字节点，而不是一个可交互控件。

---

# 5. Portable 属性中文表

当前 portable 属性：

| Canonical | 推荐中文 | 实际语义 |
|---|---|---|
| `name` | `名称` | 跨后端语义名称 |
| `key` | `逻辑键` | 跨后端稳定逻辑键 |
| `value` | `值` | 元素当前值 |
| `enabled` | `可用` | 是否可交互 |
| `visible` | `可见` | 是否可见 |
| `focused` | `已聚焦` | 是否拥有焦点 |
| `checked` | `已勾选` | 是否勾选 |
| `selected` | `已选中` | 是否选中 |

建议第一版不要把：

```text
uia.*
dom.*
```

强行全部中文化。

保留：

```aql
uia.automation_id
uia.class_name
uia.accelerator_key
uia.access_key
uia.framework_id

dom.test_id
dom.class
```

原因：

1. 它们本来就是 Backend Escape Hatch。
2. 英文 namespace 能显式提醒用户“此查询已经不再跨平台”。
3. 避免产生类似 `uia.自动化编号` 这种中英混杂但又缺少行业统一定义的语法。

编辑器 Hover 可以显示中文说明即可。

---

# 6. 操作符中文表

| Canonical | 中文 |
|---|---|
| `=` | `=` |
| `!=` | `!=` |
| `contains` | `包含` |
| `starts_with` | `开头是` |
| `ends_with` | `结尾是` |
| `matches` | `匹配` |

推荐保留：

```text
=
!=
>
>>
```

不要把这些关系运算符改成冗长文字。

其中：

```text
>
```

在 Hover / Explain 显示为：

```text
直接子元素
```

而：

```text
>>
```

显示为：

```text
任意深度后代
```

例如：

```aql
窗口(名称 包含 "微信")
    >> 按钮(名称 = "发送")
```

阅读起来已经足够直观。

---

# 7. 查询函数中文表

| Canonical | 推荐中文 |
|---|---|
| `any` | `任一` |
| `not` | `排除` |
| `first` | `第一个` |
| `nth` | `第几个` |
| `nearest` | `最近` |
| `viewport_corner` | `窗口角` |
| `viewport_edge` | `窗口边` |
| `css` | `css` |

`css` 建议不翻译。

它本身就是明确的 Browser/CDP 原生逃生口。

---

# 8. nearest 空间查询中文表

## 8.1 参数

| Canonical | 中文 |
|---|---|
| `anchor` | `锚点` |
| `target` | `目标` |
| `direction` | `方向` |
| `index` | `序号` |
| `metric` | `距离算法` |
| `position` | `位置` |
| `side` | `边` |

---

## 8.2 方向

| Canonical | 中文 |
|---|---|
| `above` | `上方` |
| `below` | `下方` |
| `left` | `左侧` |
| `right` | `右侧` |
| `any` | `任意` |

---

## 8.3 Viewport Corner

| Canonical | 中文 |
|---|---|
| `top_left` | `左上` |
| `top_right` | `右上` |
| `bottom_left` | `左下` |
| `bottom_right` | `右下` |

---

## 8.4 Viewport Edge

| Canonical | 中文 |
|---|---|
| `top` | `上` |
| `right` | `右` |
| `bottom` | `下` |
| `left` | `左` |

---

## 8.5 Distance Metric

| Canonical | 中文 |
|---|---|
| `edge_gap` | `边缘间距` |
| `center_distance` | `中心距离` |

---

# 9. 推荐的中文 AQL 最终语法

## 9.1 普通元素

英文：

```aql
button(
    name = "保存",
    enabled = true,
    visible = true
)
```

中文：

```aql
按钮(
    名称 = "保存",
    可用 = 真,
    可见 = 真
)
```

---

## 9.2 包含关系

英文：

```aql
window(name contains "微信")
    >> button(name = "发送")
```

中文：

```aql
窗口(名称 包含 "微信")
    >> 按钮(名称 = "发送")
```

---

## 9.3 Fallback

英文：

```aql
any(
    button(name = "保存"),
    button(name = "确定")
)
```

中文：

```aql
任一(
    按钮(名称 = "保存"),
    按钮(名称 = "确定")
)
```

---

## 9.4 第 N 个

英文：

```aql
nth(
    button(name = "发送"),
    2
)
```

中文：

```aql
第几个(
    按钮(名称 = "发送"),
    2
)
```

---

# 10. 最重要的视觉查询示例

现有英文：

```aql
nearest(
    anchor = text(name contains "网络结果"),
    target = text(name = $group_name),
    direction = below,
    index = 2,
    metric = edge_gap
)
```

建议中文：

```aql
最近(
    锚点 = 文字(名称 包含 "网络结果"),
    目标 = 文字(名称 = $群名),
    方向 = 下方,
    序号 = 2,
    距离算法 = 边缘间距
)
```

人可以直接读成：

> 找到“网络结果”这段文字，  
> 然后在它下方的文字候选中，  
> 按矩形边缘距离排序，  
> 选择第 2 个名称等于 `$群名` 的文字。

这就是中文化真正应该解决的问题。

---

# 11. 当前 Vision 层到底在定位什么

这是本次改造中最重要的认知。

## 11.1 当前 Vision Compiler 不是完整的 GUI 控件检测器

当前实现：

```text
crates/argusflow-vision/src/query/aql.rs
```

明确限制：

```text
Role:
    只支持 text

Property:
    只支持 name
```

也就是说当前可执行的视觉 matcher 本质是：

```aql
text(name ...)
```

而不是：

```aql
button(...)
textbox(...)
checkbox(...)
```

因此：

```aql
按钮(名称 = "发送")
```

虽然可以被公共 AQL Parser 接受，

但：

```text
Vision Compiler
```

当前不能把它解释成“屏幕上长得像按钮的区域”。

这一点必须在产品上显式展示。

---

# 12. Vision 中 `text(name=...)` 的真实含义

对 Vision 后端而言：

```aql
text(name = "发送")
```

实际接近：

```text
在当前 OCR Scene 中
寻找 normalized_text == "发送"
的 OCR 文本节点
```

OCR Scene 节点包含的核心视觉事实包括：

```text
raw_text
normalized_text
confidence
bbox
polygon/source
scene/window identity
```

因此视觉层当前真正认识的是：

> **文字 + 几何框**

而不是：

> 按钮语义 + 控件状态 + DOM 属性

---

# 13. nearest 在 Vision 层的真实含义

对于：

```aql
nearest(
    anchor = text(name = "用户名"),
    target = text(name = "输入"),
    direction = right,
    index = 1,
    metric = edge_gap
)
```

Vision 实际做的是：

```text
1. 在 OCR Scene 中查询 anchor
2. anchor 必须唯一
3. 只保留 anchor 所在窗口
4. 查询 target OCR 文本节点
5. 根据 anchor bbox 与 target bbox 判断方向
6. 计算归一化距离
7. 排序
8. 选择指定 rank
9. 如果目标距离出现完全并列，则报 AmbiguousTarget
```

对于元素锚点：

```text
direction = above / below / left / right / any
```

方向判断使用的是矩形中心位置。

距离支持：

```text
edge_gap
center_distance
```

并按 viewport 宽高归一化。

所以用户真正需要看到的解释应该是：

```text
视觉定位类型
    OCR 文字

锚点
    "用户名"

目标
    "输入"

空间限制
    右侧

排序依据
    OCR 矩形边缘间距

选择
    第 1 个
```

而不是只看到一串：

```aql
nearest(anchor=...,target=...,direction=right,index=1)
```

---

# 14. 为什么“直接把所有英文词翻成中文”是不够的

例如：

```aql
按钮(名称 = "发送")
```

对普通用户来说非常容易产生一个错误理解：

> Vision 会识别一个“按钮”。

但当前实际可能是：

```text
UIA -> 可以识别 Button ControlType

CDP -> 可以识别 button / accessibility role

Vision -> 不支持 Button role
```

因此必须把两个概念拆开：

```text
AQL 语义
≠
某个 Backend 的物理感知方式
```

AQL 表达：

```text
我要什么
```

Backend Explain 表达：

```text
我是如何找它的
```

这两个层次缺一不可。

---

# 15. 推荐新增：“中文定位解释”

建议在 AQL 编辑器下方或现有 Plan Explain / Scene Inspector 中增加：

```text
定位含义
```

例如查询：

```aql
最近(
    锚点 = 文字(名称 = "账号"),
    目标 = 文字(名称 包含 "张三"),
    方向 = 右侧,
    序号 = 1
)
```

显示：

```text
【查询含义】

目标类型
    OCR 文字

锚点
    文字名称 = "账号"

候选
    文字名称包含 "张三"

空间关系
    位于锚点右侧

排序
    按边缘间距从近到远

选择
    第 1 个
```

Backend 区域继续显示：

```text
【实际执行】

Vision / OCR
    支持

识别事实
    OCR normalized_text

几何事实
    OCR bounding box

锚点唯一性
    必须唯一

距离
    normalized edge gap
```

这样用户就不需要自己猜：

```text
nearest 到底按什么 nearest
name 到底是哪一个 name
text 到底是不是 UI 文本节点
```

---

# 16. 不应该由前端猜“视觉含义”

仓库当前架构已经强调：

```text
UI 负责展示
Planner / Compiler 负责事实
```

因此不要在 React 中写：

```ts
if (query.includes('text(')) {
    return 'OCR';
}
```

这是错误方向。

正确方向：

```text
Parser / Backend Compiler
        │
        ▼
结构化 Explain
        │
        ▼
React Renderer
```

例如可以增加：

```rust
pub enum LocatorFactKind {
    SemanticRole,
    SemanticProperty,
    OcrText,
    OcrBoundingBox,
    UiaProperty,
    DomProperty,
    SpatialDirection,
    DistanceMetric,
}
```

或更轻量：

```rust
pub struct QueryMeaning {
    pub target_summary: String,
    pub anchor_summary: Option<String>,
    pub relation_summary: Option<String>,
}
```

再由各 Backend Plan 提供：

```rust
pub struct BackendLocatorExplain {
    pub backend: BackendKind,
    pub facts: Vec<LocatorFact>,
}
```

前端只翻译结构化 code。

---

# 17. 推荐的核心架构：Dialect Layer

新增：

```text
crates/argusflow-query/src/dialect.rs
```

建议定义：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AqlDialect {
    CanonicalEn,
    ZhCn,
    Auto,
}
```

然后集中管理词表。

不要继续把词表散落在：

```text
parser.rs
language.rs
formatter.rs
MonacoAqlLanguage.ts
```

---

# 18. 推荐词表设计

示意：

```rust
pub enum SymbolKind {
    Role,
    Property,
    Function,
    Operator,
    NamedArgument,
    Direction,
    Metric,
    Corner,
    Edge,
    Boolean,
}
```

定义：

```rust
pub struct AqlSymbol {
    pub canonical: &'static str,
    pub zh_cn: &'static str,
    pub kind: SymbolKind,
}
```

例如：

```rust
AqlSymbol {
    canonical: "text",
    zh_cn: "文字",
    kind: SymbolKind::Role,
}

AqlSymbol {
    canonical: "name",
    zh_cn: "名称",
    kind: SymbolKind::Property,
}

AqlSymbol {
    canonical: "nearest",
    zh_cn: "最近",
    kind: SymbolKind::Function,
}
```

---

# 19. 为什么词表必须按“语法上下文”分类

AQL 存在重复 canonical：

```text
any
```

它既可以是：

```aql
any(query1, query2)
```

又可以是：

```aql
direction = any
```

因此不要设计：

```rust
HashMap<&str, &str>
```

这种无上下文全局映射。

应该是：

```text
Function::any
    -> 任一

Direction::any
    -> 任意
```

同理：

```text
left
```

在：

```text
direction
```

中可以显示：

```text
左侧
```

在：

```text
viewport edge
```

中显示：

```text
左
```

---

# 20. Parser 改造方式

## 20.1 不推荐

不要写成：

```rust
match role {
    "button" | "按钮" => ElementRole::Button,
    ...
}
```

然后在十几个文件复制。

短期能跑，长期一定词表漂移。

---

## 20.2 推荐

```rust
fn parse_role(name: &str) -> Option<ElementRole> {
    dialect::canonicalize_role(name)
        .and_then(parse_canonical_role)
}
```

或：

```rust
fn parse_role(name: &str) -> Option<ElementRole> {
    match lookup_symbol(SymbolKind::Role, name)?.canonical {
        "button" => Some(ElementRole::Button),
        ...
    }
}
```

同样用于：

```text
parse_attribute
parse_operator
parse_primary
parse_direction
parse_distance_metric
parse_viewport_corner
parse_viewport_edge
expect_named_argument
```

---

# 21. 布尔值

当前 lossless lexer 会特殊识别：

```text
true
false
```

如果要真正做到中文语法，建议支持：

```text
真
假
```

因此 Lexer 需要：

```rust
fn boolean_literal(text: &str) -> Option<bool> {
    match text {
        "true" | "真" => Some(true),
        "false" | "假" => Some(false),
        _ => None,
    }
}
```

中文示例：

```aql
按钮(
    可见 = 真,
    可用 = 真
)
```

Canonical 仍输出：

```aql
button(enabled=true,visible=true)
```

---

# 22. 参数名也应该支持中文

当前：

```aql
$group_name
```

属于 Identifier 规则。

修改 Unicode identifier 后建议允许：

```aql
$群名
$联系人
$搜索关键字
```

例如：

```aql
文字(名称 = $群名)
```

内部仍是：

```rust
QueryParameter {
    name: "群名"
}
```

Rust `String` 本身没有问题。

注意：

```text
bindings map key
```

也必须按原 Unicode 字符串精确匹配。

---

# 23. Formatter 的正确设计

当前 formatter 会从强类型 AST 输出：

```aql
button(
    name = "保存"
)
```

并由 canonical formatter 生成：

```aql
button(name="保存")
```

推荐拆成两个概念：

```rust
format_query_with_dialect(
    query,
    AqlDialect::ZhCn
)

canonicalize_query(query)
```

其中：

```text
format_query_with_dialect
```

面向人。

而：

```text
canonicalize_query
```

面向系统身份。

必须保证：

```text
Canonical 永远只输出英文稳定语法
```

这样：

```aql
文字(名称 = "发送")
```

和：

```aql
text(name = "发送")
```

最终都有：

```aql
text(name="发送")
```

---

# 24. 最重要的等价性约束

必须永久成立：

```rust
canonicalize(
    parse("文字(名称 = \"发送\")")
)
==
canonicalize(
    parse("text(name = \"发送\")")
)
```

以及：

```rust
parse("文字(名称 = \"发送\")")
==
parse("text(name = \"发送\")")
```

在 HIR 层完全相同。

中文不能产生一套新的：

```rust
ChineseQueryExpr
```

---

# 25. Mixed Mode 策略

推荐允许：

```aql
文字(name = "发送")
```

或：

```aql
text(名称 = "发送")
```

原因：

1. 方便渐进迁移。
2. 复制旧文档片段时不会突然报错。
3. Backend namespace 本身仍然保留英文。

但 Formatter 应统一输出当前编辑器选择的 dialect。

例如编辑器选择中文：

输入：

```aql
text(name = "发送")
```

格式化后：

```aql
文字(
    名称 = "发送"
)
```

可以增加 Information 级诊断：

```text
当前查询混用了中文和英文 AQL；格式化可统一语言。
```

不建议作为 Error。

---

# 26. Language Service 是中文化的第二个 P0

当前：

```text
crates/argusflow-query/src/language.rs
```

内仍维护了大量英文硬编码：

```text
button
textbox
window
any
nearest
name
visible
contains
below
edge_gap
...
```

包括：

```text
completions
semantic token 分类
hover suffix
word range
```

因此不能只改 Parser。

否则会出现：

```text
Runtime
    能执行中文

Editor
    全部标成 Unknown
```

必须让 Language Service 使用同一个 `dialect.rs` 词表。

---

# 27. Completion 设计

中文模式：

输入：

```text
文
```

应补全：

```text
文字()
文本框()
文档()
```

输入：

```text
文字(
```

应补全：

```text
名称
逻辑键
值
可用
可见
...
```

但可以进一步做 Backend 语义提示。

例如：

```text
文字(名称 ...)
```

Completion detail：

```text
名称
跨后端语义名称
Vision: OCR 文字内容
UIA: Accessible Name
CDP: Accessible Name / DOM 映射
```

而：

```text
按钮(...)
```

提示：

```text
Vision/OCR 当前不支持 Button role
```

这比只翻译词语更有价值。

---

# 28. Hover 设计

例如鼠标停在：

```aql
文字
```

显示：

```text
文字

Canonical: text

语义：
静态文字元素。

Vision/OCR：
当前视觉后端唯一直接支持的元素角色。
对应 OCR Scene 中的文字节点。

UIA：
映射到 Text ControlType。

CDP：
由浏览器可访问性/DOM 计划决定支持方式。
```

停在：

```aql
名称
```

显示：

```text
名称

Canonical: name

跨后端语义名称。

Vision/OCR：
匹配 OCR normalized_text。

UIA：
匹配 Name property。
```

停在：

```aql
最近
```

显示：

```text
最近

以锚点为参考，对目标候选进行方向过滤和距离排序。

Vision/OCR：
基于 OCR bounding box 计算。
```

这才真正回答“视觉层是在定位什么”。

---

# 29. Monaco 前端必须同步修改

当前：

```text
src/features/aql-editor/language/MonacoAqlLanguage.ts
```

仍有：

```text
AQL_FALLBACK_FUNCTIONS
AQL_FALLBACK_PROPERTIES
AQL_FALLBACK_OPERATORS
AQL_FALLBACK_ROLES
```

并且：

```ts
wordPattern: /[A-Za-z_][\w-]*/
```

Monarch tokenizer 也主要匹配 ASCII。

因此需要修改。

---

# 30. Monaco 最佳策略

## 最好

Rust/WASM 成为唯一语言事实来源。

Monaco 只保留：

```text
最基础的括号
字符串
正则
数字
符号
```

角色/函数/属性真正分类全部依赖：

```text
WASM semantic tokens
```

这样中文与英文不会维护两套词表。

---

## WASM 尚未加载时

Fallback 至少要支持 Unicode token：

```ts
wordPattern
```

不要继续限定：

```text
A-Z
a-z
_
```

可以使用支持 Unicode Property Escape 的表达式，例如：

```ts
/[\p{L}_][\p{L}\p{N}_.]*/u
```

参数：

```ts
/\$[\p{L}_][\p{L}\p{N}_.]*/u
```

实际需结合 Monaco 的 tokenizer API 做兼容性测试。

---

# 31. WASM 边界

当前：

```text
crates/argusflow-query-wasm/src/lib.rs
```

暴露：

```text
inspect
completions
hover
code_actions
```

推荐扩展为：

```rust
inspect(source, dialect)
completions(source, line, column, dialect)
hover(source, line, column, dialect)
```

或者让：

```text
inspect
```

自动识别 source dialect，

而 completion 使用 UI 当前语言：

```text
zh-CN
```

推荐：

```rust
pub enum LanguageDisplayDialect {
    En,
    ZhCn,
}
```

不要把产品语言与 Parse 能力绑定。

Parser 永远双语。

Display / Completion 决定用户看到哪一种。

---

# 32. 当前 SceneQueryInspector 已经具备很好的基础

当前视觉查询调试 UI 已经展示：

```text
场景布局
文字坐标
查询过程
```

并包含：

```text
文字
窗口
屏幕 BBox
置信度
AQL
结果
耗时
索引命中
扫描节点
空间候选
```

这意味着不需要重新做一套 Vision Debugger。

推荐增加第四个区域：

```text
定位解释
```

---

# 33. SceneQueryInspector 推荐新增内容

例如：

```text
定位解释

感知来源
    OCR

目标节点类型
    文字

文字条件
    包含 "发送"

锚点
    "聊天记录"

方向
    下方

距离算法
    边缘间距

空间候选
    6

最终名次
    1

命中 BBox
    x=..., y=..., w=..., h=...

OCR 置信度
    98.7%
```

如果不支持：

```text
视觉执行
    不支持

原因
    Vision 当前只支持文字(text)角色，
    当前查询要求按钮(button)角色。

可能执行后端
    Windows UIA
    Browser CDP
```

---

# 34. “视觉可执行”不要靠 Query 字符串判断

推荐 Vision Compiler / Prepared Plan 输出结构化解释。

例如：

```rust
pub struct VisionLocatorExplain {
    pub node_kind: VisionNodeKind,
    pub predicates: Vec<VisionPredicateExplain>,
    pub spatial: Option<VisionSpatialExplain>,
}
```

其中：

```rust
pub enum VisionNodeKind {
    OcrText,
}
```

未来如果真的加入：

```text
GUI 元素检测
图标检测
模板匹配
视觉语义模型
```

可以扩展：

```rust
VisualControl
Icon
Region
```

而不需要修改中文 AQL 的基本架构。

---

# 35. 推荐阶段划分

## P0：中文输入真正可解析

目标：

```aql
文字(名称 = "发送")
```

可以成功得到与：

```aql
text(name = "发送")
```

完全相同的 `UiQuery`。

改动：

```text
syntax/lexer.rs
dialect.rs   新增
parser.rs
parser/nearest.rs
tests/aql.rs
```

### P0 验收

```text
中文 role
中文 property
中文 operator
中文 function
中文 nearest 参数
中文方向
中文 metric
中文 bool
中文 parameter
```

全部可 parse。

---

## P1：中文 Formatter + Canonical 稳定

新增：

```rust
format_query_with_dialect(...)
```

保证：

```text
中文源码
    ↓
UiQuery
    ↓
canonical English
```

与英文版本一致。

### P1 验收

```rust
canonical(中文) == canonical(英文)
```

对全部语法成立。

---

## P2：编辑器完整中文体验

修改：

```text
language.rs
argusflow-query-wasm
MonacoAqlLanguage.ts
messages.ts
```

支持：

```text
中文高亮
中文补全
中文 Hover
中文诊断
中文 Formatter
Unicode word range
```

---

## P3：视觉定位解释

基于：

```text
Vision Compiler
Prepared Plan
Visual Query Trace
SceneQueryInspector
```

增加：

```text
Query Meaning
Backend Locator Explain
Vision Locator Explain
```

用户可以看到：

```text
AQL 想找什么
Vision 实际看什么
用了哪些 OCR 节点
为什么选中该 BBox
```

---

## P4：可选的高级能力

后续再考虑：

```text
中英文一键切换
混合语法提示
AQL 可视化构建器
点击 OCR Scene 自动生成中文 AQL
查询候选高亮覆盖层
nearest 候选距离排名可视化
```

---

# 36. 具体文件级改造清单

## Rust Query Engine

### `crates/argusflow-query/Cargo.toml`

新增 Unicode Identifier 依赖：

```text
unicode-ident
```

---

### `crates/argusflow-query/src/dialect.rs`

**新增。**

职责：

```text
角色词表
属性词表
函数词表
操作符词表
具名参数词表
方向词表
距离词表
viewport 词表
boolean 词表

中文 -> canonical
canonical -> 中文
completion metadata
hover metadata
```

这是整个中文化的唯一词表事实来源。

---

### `crates/argusflow-query/src/syntax/lexer.rs`

修改：

```text
ASCII identifier
    ↓
Unicode XID identifier
```

同时识别：

```text
真
假
```

以及：

```text
$中文参数
```

---

### `crates/argusflow-query/src/lexer.rs`

原则上保持：

```text
RawToken
    ↓
TokenKind
```

结构不变。

只需要确保：

```text
boolean alias
Unicode Identifier
```

lower 后语义一致。

---

### `crates/argusflow-query/src/parser.rs`

把当前散落的：

```text
parse_role
parse_attribute
parse_operator
parse_primary
```

改成使用 `dialect` registry。

不要把中英文字符串继续写死在 parser。

---

### `crates/argusflow-query/src/parser/nearest.rs`

改造：

```text
anchor / 锚点
target / 目标
direction / 方向
index / 序号
metric / 距离算法
position / 位置
side / 边

above / 上方
below / 下方
left / 左侧
right / 右侧
any / 任意

edge_gap / 边缘间距
center_distance / 中心距离
```

注意：

```text
expect_named_argument
```

当前比较的是 exact English string，

必须改成语义参数类型，而不是字符串：

```rust
expect_named_argument(NamedArgument::Anchor)
```

这是非常值得做的重构。

---

### `crates/argusflow-query/src/formatter.rs`

保留：

```rust
canonicalize_query(query)
```

英文 canonical 不变。

新增：

```rust
format_query_with_dialect(query, AqlDialect::ZhCn)
```

不要修改 canonical 的输出。

---

### `crates/argusflow-query/src/language.rs`

去掉词表重复。

以下全部从 dialect registry 获取：

```text
Completion
Semantic Token classification
Hover
Word replacement range
```

同时修改：

```text
ASCII word range
```

为 Unicode。

---

### `crates/argusflow-query/src/syntax/parser.rs`

审计 Recovery Parser 中任何：

```text
ASCII identifier
固定英文 function/property
```

确保错误恢复也认识中文。

否则会出现：

```text
Runtime parser 成功
CST / IDE recovery 异常
```

---

### `crates/argusflow-query/tests/aql.rs`

新增：

```text
bilingual equivalence
Chinese round-trip
Unicode parameter
mixed dialect
Chinese nearest
Chinese diagnostics
```

---

# 37. WASM 文件改造

### `crates/argusflow-query-wasm/src/lib.rs`

让：

```text
inspect
completions
hover
```

能够接受当前显示 dialect。

Parser 本身无需由前端决定 dialect：

```text
永远识别中英文
```

前端只决定：

```text
补全与格式化输出中文还是英文
```

---

# 38. Frontend 文件改造

### `src/features/aql-editor/language/MonacoAqlLanguage.ts`

修改：

```text
wordPattern
Monarch fallback
Fallback role/function/property list
```

推荐减少重复词表。

WASM 加载后：

```text
Rust semantic token
```

应成为主事实来源。

---

### `src/features/aql-editor/language/messages.ts`

增加中文语义说明：

```text
文字
名称
最近
方向
距离
Vision/OCR 映射
```

Hover 不只是翻译词义，

而是解释：

```text
这个词在视觉后端意味着什么
```

---

### `src/components/workflow/execution/SceneQueryInspector.tsx`

新增：

```text
定位解释
```

并展示结构化 Vision Explain。

---

### `src/components/workflow/.../PlanExplanation.tsx`

如果当前 Backend Explain 已经存在，

建议增加：

```text
语义目标
后端映射
感知来源
```

不要让 Scene Inspector 和 Plan Explain 重复维护语义。

---

# 39. UIA / CDP / Vision Compiler 是否需要改中文？

**不需要。**

例如：

```rust
ElementRole::Button
SelectorAttribute::Name
MatchOperator::Contains
```

继续保持不变。

UIA Compiler 继续：

```text
ElementRole -> UIA ControlType
SelectorAttribute -> UIA Property
```

CDP Compiler 继续：

```text
ElementRole -> DOM / AX semantics
```

Vision Compiler 继续：

```text
ElementRole::Text
SelectorAttribute::Name
    ↓
OCR normalized_text
```

这就是方言层方案最大的价值。

---

# 40. Vision Compiler 推荐增强，但不是为了“支持中文”

建议增强的原因是 Explain。

当前 Vision Compile 错误主要是字符串描述，例如：

```text
OCR scenes only contain text role nodes
OCR text queries only expose the portable name property
```

建议逐步结构化：

```rust
pub enum VisionUnsupportedReason {
    RoleUnsupported {
        requested: ElementRole,
        supported: Vec<ElementRole>,
    },
    PropertyUnsupported {
        requested: SelectorAttribute,
    },
    ExpressionUnsupported {
        kind: QueryExprKind,
    },
}
```

UI 可以本地化成：

```text
视觉层不支持“按钮”角色；
当前 Vision Scene 只有 OCR 文字节点。
```

比字符串拼接更稳定。

---

# 41. 需要明确展示的 Vision 支持矩阵

当前实现建议 UI 直接展示：

| AQL 能力 | Vision/OCR |
|---|---|
| `text(...)` | ✅ |
| `name` | ✅ |
| `=` | ✅ |
| `!=` | ✅ |
| `contains` | ✅ |
| `starts_with` | ✅ |
| `ends_with` | ✅ |
| `matches` | ✅ |
| `any(...)` | ✅ |
| `first(...)` | ✅ |
| `nth(...)` | ✅ |
| `nearest(...)` | ✅ |
| `button(...)` | ❌ |
| `textbox(...)` | ❌ |
| `visible` | ❌ |
| `enabled` | ❌ |
| `>` | ❌ |
| `>>` | ❌ |
| `not(...)` | ❌ |
| `css(...)` | ❌ |

这张表对理解视觉能力非常重要。

---

# 42. 推荐增加“视觉模式提示”

当用户输入：

```aql
文字(名称 = "发送")
```

编辑器可以显示：

```text
Vision 可直接执行
OCR 文字精确匹配
```

输入：

```aql
按钮(名称 = "发送")
```

显示：

```text
Vision 当前不可执行
UIA/CDP 可能可执行
```

输入：

```aql
最近(...)
```

显示：

```text
Vision 可执行空间查询
依据：OCR BBox
```

这里必须使用：

```text
Planner / Compiler 返回的数据
```

而不是前端自己猜。

---

# 43. 推荐的“视觉定位阅读模式”

除了源码编辑器，建议提供一个只读视图：

```text
源码 | 中文解释
```

例如：

```aql
最近(
    锚点 = 文字(名称 包含 "网络结果"),
    目标 = 文字(名称 = $群名),
    方向 = 下方,
    序号 = 2
)
```

解释成：

```text
从 OCR 场景中：

1. 找到文字中包含“网络结果”的唯一锚点；
2. 只在该锚点所在窗口继续查找；
3. 找到文字名称等于变量“群名”的候选；
4. 排除不在锚点下方的候选；
5. 按矩形边缘间距排序；
6. 选择第 2 个；
7. 如果该距离排名出现几何并列，则判为定位歧义。
```

这比单纯中文关键字更能提高理解速度。

---

# 44. Query Meaning 建议从 AST 生成

可以在：

```text
argusflow-query
```

中新增：

```rust
explain_semantics(query: &UiQuery) -> QuerySemanticExplain
```

结构例如：

```rust
pub enum QuerySemanticExplain {
    Match {
        role: ElementRole,
        predicates: Vec<PredicateExplain>,
    },
    Descendant {
        ancestor: Box<QuerySemanticExplain>,
        target: Box<QuerySemanticExplain>,
    },
    Nearest {
        anchor: AnchorExplain,
        target: Box<QuerySemanticExplain>,
        direction: SpatialDirection,
        index: usize,
        metric: DistanceMetric,
    },
    ...
}
```

注意：

```text
这里只解释“查询想表达什么”
```

不要解释：

```text
某 Backend 怎么执行
```

Backend 执行解释仍由 Backend Compiler / PreparedPlan 提供。

---

# 45. 最终 UI 建议形成两层 Explain

## Query Semantic Explain

回答：

```text
我想找什么？
```

例如：

```text
下方最近的第 2 个指定文字
```

---

## Backend Locator Explain

回答：

```text
当前后端具体怎么看？
```

Vision：

```text
OCR normalized_text + bbox
```

UIA：

```text
ControlType + Name Property + UIA Tree
```

CDP：

```text
DOM / AX Tree + browser selector
```

最终用户才能理解：

```text
同一个 AQL
为什么不同后端行为不同
```

---

# 46. 中文 AQL 与 Canonical 的完整示例

中文：

```aql
任一(
    最近(
        锚点 = 文字(名称 包含 "网络结果"),
        目标 = 文字(名称 = $群名),
        方向 = 下方,
        序号 = 1,
        距离算法 = 边缘间距
    ),
    第一个(
        按钮(
            名称 = "打开",
            可用 = 真
        )
    )
)
```

内部 AST 不区分中文。

Canonical：

```aql
any(nearest(anchor=text(name contains "网络结果"),target=text(name=$群名),direction=below,index=1,metric=edge_gap),first(button(enabled=true,name="打开")))
```

Planner 可以：

```text
Branch 0:
    Vision 可执行

Branch 1:
    UIA/CDP 可执行
```

中文只影响阅读，不影响 fallback 语义。

---

# 47. 是否要修改 AQL Language Version？

建议：

> **不要因为中文方言本身改变 Query Semantics Version。**

中文/英文属于：

```text
Dialect
```

而：

```text
V1 / V2
```

应该继续表达：

```text
语法/语义能力版本
```

而不是 UI 语言。

但是有一个兼容性问题：

```text
老版本 ArgusFlow
```

不认识中文源码。

因此有两种策略。

---

# 48. 持久化策略 A：直接保存中文源码

```text
AqlQuery.source = 中文源码
```

优点：

```text
用户看到的就是保存的
Git Diff 可读
```

缺点：

```text
旧 ArgusFlow 无法解析
```

如果项目不要求老版本打开新工作流，这是最简单的方式。

---

# 49. 持久化策略 B：兼容模式

如果强烈要求旧版本兼容：

编辑器允许中文输入，

保存前：

```text
中文
 ↓ parse
UiQuery
 ↓ canonical
英文 canonical source
```

工作流中存：

```aql
text(name="发送")
```

编辑器加载后再：

```text
parse
 ↓
format ZhCn
```

显示：

```aql
文字(
    名称 = "发送"
)
```

这样 Runtime 和旧系统看到的仍是英文。

代价是：

```text
用户输入的原始排版不会作为事实源保存
```

当前 AQL 又没有评论语法，因此这个代价相对可控。

---

# 50. 我的建议

如果 ArgusFlow 仍处于快速迭代阶段：

```text
选择策略 A
```

即直接支持双语源码并保存用户实际输入。

同时：

```text
canonical identity 永远英文
```

如果以后要做稳定跨版本 workflow exchange，

再增加：

```text
canonical source export
```

即可。

---

# 51. 测试方案

## 51.1 Role 等价

```rust
assert_eq!(
    parse_query(r#"文字(名称 = "发送")"#)?,
    parse_query(r#"text(name = "发送")"#)?,
);
```

---

## 51.2 Boolean

```rust
assert_eq!(
    parse_query("按钮(可见 = 真)")?,
    parse_query("button(visible = true)")?,
);
```

---

## 51.3 Operator

```rust
assert_eq!(
    parse_query(r#"窗口(名称 包含 "微信")"#)?,
    parse_query(r#"window(name contains "微信")"#)?,
);
```

---

## 51.4 Unicode Parameter

```rust
let query = parse_query(
    r#"文字(名称 = $群名)"#
)?;
```

并确保：

```text
query_parameter_names
```

返回：

```text
群名
```

---

## 51.5 nearest

```rust
assert_eq!(
    parse_query(
        r#"
        最近(
            锚点 = 文字(名称 = "A"),
            目标 = 文字(名称 = "B"),
            方向 = 下方,
            序号 = 1,
            距离算法 = 边缘间距
        )
        "#
    )?,
    parse_query(
        r#"
        nearest(
            anchor = text(name = "A"),
            target = text(name = "B"),
            direction = below,
            index = 1,
            metric = edge_gap
        )
        "#
    )?,
);
```

---

## 51.6 Canonical Identity

```rust
let zh = parse_query(...)?;
let en = parse_query(...)?;

assert_eq!(
    canonicalize_query(&zh),
    canonicalize_query(&en),
);
```

---

## 51.7 Formatter Round Trip

```text
中文源码
 ↓ parse
中文 formatter
 ↓ parse
```

最终 AST 必须一致。

---

## 51.8 Mixed Mode

```aql
文字(name = "发送")
```

必须能解析。

Formatter ZhCn 输出：

```aql
文字(
    名称 = "发送"
)
```

---

# 52. Vision 专项测试

必须新增：

```text
中文源码 parse
      ↓
UiQuery
      ↓
Vision Compiler
```

确保中文与英文完全等价。

### 应通过

```aql
文字(名称 = "发送")
```

```aql
任一(
    文字(名称 = "保存"),
    文字(名称 = "确定")
)
```

```aql
最近(
    锚点 = 文字(名称 = "用户名"),
    目标 = 文字(名称 = "张三"),
    方向 = 右侧,
    序号 = 1
)
```

---

### Vision 应拒绝

```aql
按钮(名称 = "发送")
```

```aql
文字(可见 = 真)
```

```aql
窗口()
    >> 文字(名称 = "发送")
```

```aql
排除(
    文字(名称 = "取消")
)
```

并确保错误信息明确说明：

```text
不是中文语法问题
而是 Vision Backend 能力不支持
```

---

# 53. Editor 专项测试

测试：

```text
中文 token range
UTF-16 column
中文 completion replacement
Hover range
diagnostic underline
中文参数
中英文混排
IME composition
```

中文字符是多字节 UTF-8，

而 Monaco 使用 UTF-16 column。

当前代码已经显式做：

```text
byte range
    ↕
UTF-16 editor range
```

因此必须确保 Unicode identifier 改造后继续使用已有范围转换函数，

不能用：

```text
字符数 == byte offset
```

这种错误假设。

---

# 54. 风险清单

## 风险 1：Parser 支持中文，但 Lossless Lexer 不支持

这是最高概率问题。

解决：

```text
先改 Unicode Lexer
再改 Parser
```

---

## 风险 2：Runtime 支持，但 Editor 不支持

解决：

```text
dialect registry
```

必须成为 Rust 唯一词表来源。

---

## 风险 3：Formatter 把中文重新格式化成英文

解决：

```text
human formatter 带 dialect
canonical formatter 固定英文
```

两者分离。

---

## 风险 4：Visual 语义被中文角色误导

例如：

```aql
按钮(...)
```

看起来像视觉按钮检测。

解决：

```text
Backend Locator Explain
```

明确显示：

```text
Vision 不支持此 role
```

---

## 风险 5：中文同义词越来越多

例如：

```text
文本
文字
静态文字
标签
```

都想表示 `text`。

第一版禁止无限 aliases。

建议只保留一个推荐拼写：

```text
text -> 文字
```

以后如果要兼容同义词，

可以解析但 Formatter 永远输出主词。

---

## 风险 6：翻译 Backend-specific namespace

例如：

```text
uia.自动化ID
dom.测试ID
```

会增加大量非必要语法。

第一版建议完全不做。

---

## 风险 7：中文关键字与自由变量冲突

当前 AQL 函数、role、property 都有明确语法位置，

所以冲突总体可控。

但 registry 应按：

```text
Role
Function
Property
Direction
...
```

分类解析，

不要做全局 keyword token。

---

# 55. 推荐的最小实现顺序

如果现在就开始改代码，我建议严格按照：

```text
1. dialect.rs
2. Unicode lossless lexer
3. parser role/property/operator/function aliases
4. nearest aliases
5. bilingual parser tests
6. canonical identity tests
7. Chinese formatter
8. language service
9. WASM
10. Monaco
11. Query Semantic Explain
12. Vision Locator Explain
13. SceneQueryInspector
```

不要先从 React 界面翻译开始。

---

# 56. 第一版可以不做的事情

为了避免范围失控，第一版不要做：

```text
中文 CSS
中文正则语法
中文 UIA namespace
中文 DOM namespace
中文注释语法
自然语言 AQL
LLM 自动重写 AQL
视觉按钮分类
图标识别
模板匹配
```

这些都和“让当前 AQL 更容易理解”不是同一个问题。

---

# 57. 推荐的第一版最终体验

用户进入 AQL Editor，默认中文：

```aql
最近(
    锚点 = 文字(名称 包含 "联系人"),
    目标 = 文字(名称 = $联系人名称),
    方向 = 下方,
    序号 = 1
)
```

下方立即显示：

```text
查询含义
────────────────────────
在“联系人”文字下方，
查找名称等于变量“联系人名称”的文字，
按边缘距离排序，
选择第 1 个。
```

再下面：

```text
后端执行
────────────────────────
Vision / OCR       可执行
Windows UIA        根据当前计划判断
Browser CDP        根据当前计划判断
```

点击 Vision：

```text
视觉定位依据
────────────────────────
感知类型      OCR
节点          OCR 文字
匹配字段      normalized_text
方向依据      OCR BBox 中心
距离依据      BBox edge gap
锚点要求      唯一
空间候选      5
最终命中      1
置信度        98.2%
```

这会比单纯：

```text
把 nearest 翻成 最近
```

提升大得多。

---

# 58. 推荐的最终目录变化

```text
crates/
└─ argusflow-query/
   └─ src/
      ├─ dialect.rs                 # 新增：双语词表唯一事实源
      ├─ lexer.rs
      ├─ parser.rs
      ├─ parser/
      │  └─ nearest.rs
      ├─ formatter.rs
      ├─ language.rs
      └─ semantic_explain.rs        # 可选新增

crates/
└─ argusflow-vision/
   └─ src/
      └─ query/
         ├─ aql.rs
         └─ aql/
            └─ spatial.rs
            # 增加结构化 locator explain，不改变中文解析

src/
├─ features/
│  └─ aql-editor/
│     └─ language/
│        ├─ MonacoAqlLanguage.ts
│        └─ messages.ts
└─ components/
   └─ workflow/
      └─ execution/
         └─ SceneQueryInspector.tsx
```

---

# 59. 建议的 API 草案

```rust
pub enum AqlDialect {
    CanonicalEn,
    ZhCn,
}

pub fn parse_query(source: &str) -> Result<UiQuery, AqlError>;

pub fn format_query(
    query: &UiQuery,
) -> String;

pub fn format_query_with_dialect(
    query: &UiQuery,
    dialect: AqlDialect,
) -> String;

pub fn canonicalize_query(
    query: &UiQuery,
) -> String;

pub fn explain_query_semantics(
    query: &UiQuery,
) -> QuerySemanticExplain;
```

核心原则：

```text
parse_query
    双语

format_query_with_dialect
    给人看

canonicalize_query
    给系统看
```

---

# 60. 推荐的 Dialect API 草案

```rust
pub enum SymbolContext {
    Role,
    Function,
    Property,
    Operator,
    NamedArgument,
    Direction,
    Metric,
    ViewportCorner,
    ViewportEdge,
    Boolean,
}

pub fn to_canonical(
    context: SymbolContext,
    source: &str,
) -> Option<&'static str>;

pub fn display(
    context: SymbolContext,
    canonical: &str,
    dialect: AqlDialect,
) -> Option<&'static str>;
```

Parser 不直接知道：

```text
中文词表
```

只知道：

```text
我要一个 Role
```

这样结构最干净。

---

# 61. 一个更进一步的改进：Role Support Hint

Language Service 可以拿到：

```text
QueryBackend
```

支持矩阵后，

Completion 显示：

```text
文字
  Vision ✓
  UIA ✓
  CDP ...

按钮
  Vision ×
  UIA ✓
  CDP ✓
```

这样用户在写 AQL 时就知道：

> 这条查询是在依赖“语义控件树”，还是可以进入“视觉 OCR”。

这是最适合 ArgusFlow 的语言体验之一。

---

# 62. 一个非常重要的命名建议

不要把 Vision 翻译成模糊的：

```text
视觉识别
```

建议 UI 区分：

```text
Vision / OCR
```

当前事实：

```text
OCR 文字场景
```

未来如果增加：

```text
GUI 元素检测
Icon detector
Visual grounding
```

再显示：

```text
Vision / OCR Text
Vision / GUI Element
Vision / Icon
```

否则用户很容易以为当前 Vision 已经具备通用视觉 grounding。

---

# 63. 实施完成后的验收标准

## 语言层

- [ ] `文字(名称 = "发送")` 可直接解析。
- [ ] `$群名` 可作为参数。
- [ ] `真/假` 可作为布尔值。
- [ ] 中文 `nearest` 全部参数可用。
- [ ] 中英文可以混合输入。
- [ ] 中文和英文生成完全相同的 `UiQuery`。
- [ ] 中文和英文生成完全相同的 canonical identity。
- [ ] Canonical 仍保持稳定英文格式。

## 编辑器

- [ ] 中文关键字正确高亮。
- [ ] 中文补全替换范围正确。
- [ ] 中文 Hover 正确。
- [ ] Unicode 诊断范围正确。
- [ ] 格式化可统一成中文。
- [ ] WASM 未加载时 fallback 不把中文全部当 Unknown。

## Vision

- [ ] `文字(名称=...)` 能编译到 Vision。
- [ ] `按钮(...)` 在 Vision 上明确显示 Unsupported。
- [ ] Explain 明确说明 Vision 当前匹配 OCR `normalized_text`。
- [ ] nearest 明确说明使用 OCR BBox。
- [ ] Scene Inspector 展示锚点、候选、最终命中和空间排序依据。

## 架构

- [ ] UIA Compiler 无需识别中文字符串。
- [ ] CDP Compiler 无需识别中文字符串。
- [ ] Vision Compiler 无需识别中文字符串。
- [ ] Planner 不读取中文关键字。
- [ ] React 不通过字符串规则判断 Backend 能力。
- [ ] Dialect registry 是中文词表唯一事实来源。

---

# 64. 最终建议

如果你的核心目标是：

> “让我一眼知道视觉层到底在定位什么”

那么优先级应该是：

```text
第一：
中文 Surface Syntax

第二：
Query Semantic Explain

第三：
Vision Locator Explain

第四：
Scene 候选可视化
```

而不是：

```text
把所有 Rust enum / backend code 改成中文
```

最值得落地的最终模型是：

```text
中文 AQL
    ↓
“我想找什么”

Canonical UiQuery
    ↓
“机器理解的唯一语义”

Backend Plan
    ↓
“当前后端怎么找”

Vision Trace
    ↓
“这一次实际看到了哪些文字框，
为什么最终点了这个位置”
```

这样 AQL 才会从：

```text
查询字符串
```

真正变成：

```text
可解释的 UI 定位语言
```

---

# 65. 建议第一批直接落地的中文关键字

建议第一批冻结以下词汇，不要频繁改名：

```text
角色
────────────────
窗口
对话框
面板
按钮
文本框
复选框
单选框
下拉框
列表
列表项
树
树节点
选项卡
选项卡项
菜单
菜单项
链接
图像
表格
行
单元格
文档
文字

属性
────────────────
名称
逻辑键
值
可用
可见
已聚焦
已勾选
已选中

函数
────────────────
任一
排除
第一个
第几个
最近
窗口角
窗口边

操作符
────────────────
包含
开头是
结尾是
匹配

空间
────────────────
锚点
目标
方向
序号
距离算法
位置
边

方向
────────────────
上方
下方
左侧
右侧
任意

距离
────────────────
边缘间距
中心距离

布尔
────────────────
真
假
```

---

# 66. 最终示例：推荐作为 README 的中文 AQL 示例

```aql
最近(
    锚点 = 文字(
        名称 包含 "搜索结果"
    ),
    目标 = 文字(
        名称 = $联系人
    ),
    方向 = 下方,
    序号 = 1,
    距离算法 = 边缘间距
)
```

建议旁边直接显示：

```text
视觉含义：

以 OCR 识别到的“搜索结果”文字框作为唯一锚点，
在同一窗口中查找文字等于变量“联系人”的 OCR 节点，
只保留位于锚点下方的候选，
按两个文字框之间的归一化边缘距离排序，
选择最近的第 1 个。
```

这应该成为中文 AQL 的设计标杆。

---

# 67. 本次审计涉及的核心文件

```text
crates/argusflow-core/src/query.rs

crates/argusflow-query/src/
    language.rs
    lexer.rs
    parser.rs
    parser/nearest.rs
    formatter.rs
    capability.rs
    resolve.rs
    syntax/lexer.rs

crates/argusflow-query/tests/aql.rs

crates/argusflow-query-wasm/src/lib.rs

crates/argusflow-windows/src/uia/compiler.rs

crates/argusflow-vision/src/
    query/aql.rs
    query/aql/spatial.rs
    scene/model.rs

crates/argusflow-agent/src/router.rs

src/features/aql-editor/language/
    MonacoAqlLanguage.ts

src/components/workflow/execution/
    SceneQueryInspector.tsx

docs/
    ArgusFlow AQL 审计与重构方案.md
```

---

# 68. 一句话实施原则

> **中文化 Parser，不中文化 IR；中文化 Editor，不中文化 Backend；视觉含义由 Compiler/Trace 解释，不由 UI 猜。**

