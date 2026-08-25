# ArgusFlow 对接真实 Windows UI Automation（UIA）实施方案

> **目标项目**：`SLOE-debug/argusflow`  
> **基线分支**：`main`  
> **基线提交**：`e74e99a68a1b673d1efa1fcce277e2769f30d1c3`（`refactor: 重构 AQL 语言服务与执行规划`）  
> **目标平台**：`x86_64-pc-windows-msvc` / 64 位 Windows  
> **真实 UIA 验收应用**：Notepad++（建议 64 位、英文 UI、禁用插件、独立 session）  
> **方案定位**：在当前 AQL / PreparedPlan / ActionRouter 架构上补齐真实 UIA executor，不重做 AQL，不把 UIA 逻辑塞进 Tauri 入口，不用 SendInput 冒充 UIA。

---

## 1. 结论

当前项目已经完成了 UIA 接入最重要的“上半段”：

```text
AQL Source
   ↓
parse_stored_query
   ↓
UiQuery / QueryExpr
   ↓
normalize
   ↓
compile_uia_query
   ↓
UiaQueryPlan
   ↓
PreparedCandidate / PreparedPlan
```

真正缺失的是：

```text
UiaQueryPlan
   ↓
真实 Windows UI Automation client
   ↓
IUIAutomationElement 查询
   ↓
residual filter
   ↓
唯一目标解析
   ↓
InvokePattern / ValuePattern
   ↓
ActionOutcome
```

当前 `crates/argusflow-windows/src/uia/mod.rs` 中 `UiaPreparedExecution::execute()` 固定返回：

```text
Windows UI Automation 尚未接入
```

同时 UIA candidate 的：

```rust
availability: RuntimeAvailability::NotImplemented
```

也是硬编码。

因此，本次对接**不应新建另一套 selector/UIA DSL，也不应绕开 ActionRouter**。正确做法是在现有 `argusflow-windows::uia` 内实现一个真实 UIA runtime + executor，并让现有 `UiaBackend::prepare()` 冻结的 `UiaQueryPlan` 在执行阶段被直接物化执行。

---

# 2. 必须继续遵守现有 docs 的四条架构原则

本方案以项目现有 `docs` 为约束，而不是另起架构。

## 2.1 Backend Compiler 继续作为能力事实来源

现有 docs 已明确：

> Backend Compiler 是能力事实来源。

所以 executor 不允许到运行时再决定：

```text
这个 role 到底能不能映射
这个 attribute 到底是不是 native
这个 operator 到底是不是 residual
```

这些都应尽量在 `compile_uia_query()` 时确定。

最终边界应是：

```text
AQL
 ↓
UIA Compiler
 ↓
已经确认 UIA 可表达的强类型 UiaQueryPlan
 ↓
Executor 只执行，不重新猜语义
```

---

## 2.2 PreparedPlan 与 Executor 必须共享同一份计划

当前 `PreparedExecution` 已明确约束：

```rust
async fn execute(&self) -> Result<ActionOutcome, AutomationError>;
```

并要求：

```text
禁止重新解析或重新规划原始动作
```

所以真实 UIA executor 只允许对冻结的 `UiaQueryPlan` 做：

```text
native materialization
```

例如把：

```text
UiaNativeProperty::Name
```

物化成：

```text
UIA_NamePropertyId
```

把：

```text
UiaControlType::Button
```

物化成：

```text
UIA_ButtonControlTypeId
```

这属于后端执行，不属于重新 plan。

---

## 2.3 ExecutionContext 必须进入 UIA prepare

现有 `ExecutionContext` 已经有：

```rust
foreground_window
active_process
browser_session
accessibility
visual_cache
```

UIA 必须继续使用它。

尤其要在 prepare 阶段冻结：

```text
HWND
process_id
UIA runtime availability
```

不能 execute 时重新读取前台窗口，否则：

```text
Planner Explain 指向 Notepad++
用户执行前切换了窗口
Executor 却自动操作了另一个窗口
```

会破坏 PreparedPlan 的事实一致性。

---

## 2.4 Semantic Support 与 Runtime Availability 继续拆开

UIA query 即使能编译为：

```text
Native / Low Cost
```

也不等于当前 UIA runtime 一定可用。

真实接入后：

```text
SupportLevel
```

仍由 compiler 决定；

```text
RuntimeAvailability
```

改由 UIA runtime + ExecutionContext 决定：

```text
Ready
MissingContext
Unavailable
```

真实 executor 接入后不再出现：

```text
NotImplemented
```

除非构建时明确裁剪了 UIA executor。

---

# 3. 当前代码现状与缺口

当前仓库已经存在：

```text
crates/argusflow-windows/
├─ Cargo.toml
├─ src/
│  ├─ context.rs
│  ├─ input/
│  ├─ capture/
│  ├─ window/
│  └─ uia/
│     ├─ mod.rs
│     ├─ compiler.rs
│     ├─ explain.rs
│     └─ plan.rs
└─ tests/
   └─ uia_query_compiler.rs
```

而且 `Cargo.toml` 已启用：

```toml
Win32_System_Com
Win32_UI_Accessibility
Win32_UI_WindowsAndMessaging
```

因此 P0 不需要换库，继续使用当前 workspace 已锁定的：

```toml
windows = "0.62.2"
```

即可。

---

# 4. 推荐的最终模块结构

不要继续把实现堆进 `uia/mod.rs`。

建议重构为：

```text
crates/argusflow-windows/src/uia/
├─ mod.rs
├─ backend.rs
├─ runtime.rs
├─ native.rs
├─ condition.rs
├─ cache.rs
├─ executor.rs
├─ action.rs
├─ property.rs
├─ error.rs
├─ compiler.rs
├─ plan.rs
└─ explain.rs
```

职责：

| 文件 | 职责 |
|---|---|
| `mod.rs` | 只做模块声明和最小 public export |
| `backend.rs` | `UiaBackend`、`ActionBackend::prepare()`、`UiaPreparedExecution` |
| `runtime.rs` | UIA 专用 COM worker thread、请求队列、runtime health |
| `native.rs` | ArgusFlow UIA 强类型 native IR，不暴露裸 property/control type 数字 |
| `condition.rs` | `UiaNativePredicate -> IUIAutomationCondition` |
| `cache.rs` | `UiaCachedProperty -> IUIAutomationCacheRequest` |
| `executor.rs` | 递归执行 `UiaPlanExpr`、关系 scope、Any/First/Nth、唯一性校验 |
| `action.rs` | `Click -> InvokePattern`、`SetValue -> ValuePattern` |
| `property.rs` | 读取 cached/current property、类型转换、residual evaluator |
| `error.rs` | Windows HRESULT / stale element / worker 状态到内部 `UiaError` |
| `compiler.rs` | AQL -> 真正可执行的 UIA plan |
| `plan.rs` | UIA backend plan 类型 |
| `explain.rs` | 从实际 plan 生成 explain |

这符合仓库 `AGENTS.md` 对：

```text
高内聚
低耦合
mod.rs 保持精简
unsafe 最小化
强类型优先
```

的要求。

---

# 5. 最重要的设计：UIA COM 只能活在专用 worker 线程

## 5.1 不建议把 `IUIAutomation` 直接放进 async executor

当前：

```rust
PreparedExecution: Send + Sync
```

而实际 workflow 运行在 Tokio runtime 上。

如果直接：

```rust
struct UiaBackend {
    automation: IUIAutomation,
}
```

再让 async task 在不同 Tokio worker thread 上调用 COM，会把 COM apartment、线程切换和对象生命周期混在一起。

这会让后续出现非常难排查的问题：

```text
CoInitializeEx 在哪个线程执行？
COM interface 在哪个线程创建？
execute await 前后是否换线程？
Provider call 卡住时是否阻塞 Tokio worker？
```

因此推荐：**所有 UIA COM 对象都不离开专用 OS 线程。**

---

## 5.2 UIA Runtime 结构

建议：

```rust
pub struct UiaRuntime {
    sender: UiaRequestSender,
    health: Arc<UiaRuntimeHealth>,
}
```

内部启动一个：

```text
argusflow-uia
```

线程。

线程入口：

```text
CoInitializeEx(COINIT_MULTITHREADED)
        ↓
CoCreateInstance(CUIAutomation8)
        ↓
IUIAutomation client
        ↓
request loop
        ↓
CoUninitialize
```

建议优先：

```text
CUIAutomation8
```

当前项目只面向现代 64 位 Windows，不需要在 P0 再维护老 UIAutomation client 的兼容分支。

---

## 5.3 请求模型

不要让 `IUIAutomationElement` 穿过 channel。

发送的应该全部是 ArgusFlow 自己的 `Send` 数据：

```rust
pub(crate) struct UiaExecuteRequest {
    pub window: PreparedWindowTarget,
    pub action: AutomationAction,
    pub query_plan: UiaQueryPlan,
}
```

worker 内部才创建：

```text
IUIAutomationElement
IUIAutomationCondition
IUIAutomationCacheRequest
IUIAutomationInvokePattern
IUIAutomationValuePattern
```

然后在 worker 内销毁。

---

## 5.4 async 边界

推荐：

```text
Tokio task
  │
  │ send UiaExecuteRequest
  ▼
UIA OS worker thread
  │
  │ synchronous COM/UIA calls
  ▼
oneshot result
  │
  ▼
Tokio task resumes
```

可以在 `argusflow-windows` 增加 workspace 已经存在的：

```toml
tokio.workspace = true
```

只使用：

```text
tokio::sync::oneshot
```

或一个轻量 typed channel。

不要在 async `execute()` 里直接执行同步 UIA COM 查询。

---

# 6. `UiaBackend` 从无状态 unit struct 改成持有 runtime

当前：

```rust
pub struct UiaBackend;
```

建议改成：

```rust
pub struct UiaBackend {
    runtime: Arc<UiaRuntime>,
}
```

并提供：

```rust
impl UiaBackend {
    pub fn new(runtime: Arc<UiaRuntime>) -> Self {
        Self { runtime }
    }
}
```

`UiaPreparedExecution` 建议变成：

```rust
struct UiaPreparedExecution {
    runtime: Arc<UiaRuntime>,
    window: PreparedWindowTarget,
    action: AutomationAction,
    query_plan: UiaQueryPlan,
}
```

注意这里保存的是：

```text
冻结后的 HWND / PID
冻结后的 action
冻结后的 UiaQueryPlan
```

不是：

```text
IUIAutomationElement
```

---

# 7. prepare 阶段如何判断真实 availability

建议定义：

```rust
pub enum UiaRuntimeState {
    Ready,
    Failed(UiaInitFailure),
}
```

或者对外只暴露只读 health snapshot。

`UiaBackend::prepare()`：

```text
UIA compiler 成功？
    no -> PlanRejection::Unsupported
    yes
      ↓
UIA runtime 初始化成功？
    no -> RuntimeAvailability::Unavailable
    yes
      ↓
ExecutionContext 有 foreground_window？
    no -> RuntimeAvailability::MissingContext
    yes
      ↓
RuntimeAvailability::Ready
```

因此 `PlanExplain` 最终可能是：

```text
backend: WindowsUia
support: Native
cost: Low
availability: Ready
context_fitness: Good
```

而不是现在的：

```text
availability: NotImplemented
```

---

# 8. `WindowsExecutionContextProvider` 也要接 UIA runtime health

当前 `WindowsExecutionContextProvider` 只读取：

```text
GetForegroundWindow
GetWindowThreadProcessId
```

然后：

```rust
..ExecutionContext::default()
```

导致：

```rust
accessibility.ready == false
```

建议改成：

```rust
pub struct WindowsExecutionContextProvider {
    uia_health: Arc<UiaRuntimeHealth>,
}
```

`snapshot()`：

```rust
ExecutionContext {
    foreground_window: foreground_window_context(),
    accessibility: AccessibilityContext {
        ready: self.uia_health.is_ready(),
    },
    ...
}
```

这样：

```text
ExecutionContext
Planner Explain
UiaBackend availability
```

三者才是一致事实。

---

# 9. Tauri 装配方式

当前 `src-tauri/src/runtime.rs` 直接：

```rust
Arc::new(UiaBackend)
```

建议改成：

```text
AppState::new
   ↓
create UiaRuntime
   ↓
Arc<UiaRuntime>
   ├─ UiaBackend::new(...)
   └─ WindowsExecutionContextProvider::new(...)
```

示意：

```rust
let uia_runtime = Arc::new(UiaRuntime::start());

let backends: Vec<Arc<dyn ActionBackend>> = vec![
    Arc::new(UiaBackend::new(uia_runtime.clone())),
    Arc::new(CdpBackend),
    ...
];

let context_provider = Arc::new(
    WindowsExecutionContextProvider::new(uia_runtime.health())
);
```

UIA 初始化失败**不建议让整个 ArgusFlow 启动失败**。

更合理的是：

```text
ArgusFlow 正常启动
UIA candidate:
    support = Native
    availability = Unavailable
    diagnostics = UIA runtime initialization failed
```

这正好符合项目现有 `SupportLevel / RuntimeAvailability` 分离设计。

---

# 10. 必须先修正 UIA compiler 的 native 映射边界

这是本次真实接 UIA 时最容易被忽略的问题。

当前 `UiaMatcherPlan` 保存：

```rust
pub role: ElementRole,
pub pushdown: Vec<PropertyPredicate>,
pub cache: Vec<SelectorAttribute>,
pub residual: Vec<PropertyPredicate>,
```

这意味着 executor 仍要现场回答：

```text
ElementRole::Dialog 怎么映射？
ElementRole::Row 怎么映射？
SelectorAttribute::Visible 怎么映射？
SelectorAttribute::Checked 怎么映射？
```

这会让 executor 变成第二个 capability analyzer。

建议把 plan 再降一层。

---

# 11. 推荐增加 UIA Native IR

## 11.1 Control Type

```rust
pub enum UiaControlType {
    Window,
    Pane,
    Button,
    Edit,
    CheckBox,
    RadioButton,
    ComboBox,
    List,
    ListItem,
    Tree,
    TreeItem,
    Tab,
    TabItem,
    Menu,
    MenuItem,
    Hyperlink,
    Image,
    Table,
    Document,
    Text,
}
```

executor 才负责最后映射成：

```text
UIA_WindowControlTypeId
UIA_ButtonControlTypeId
UIA_EditControlTypeId
...
```

---

## 11.2 Role Constraint

因为 AQL role 和 UIA ControlType 并非永远 1:1，建议：

```rust
pub enum UiaRoleConstraint {
    ControlType(UiaControlType),
    Dialog,
}
```

其中：

```text
dialog
```

不要简单等价成：

```text
Window
```

建议编译成：

```text
ControlType = Window
AND
IsDialog = true
```

即使用：

```text
UIA_IsDialogPropertyId
```

来保持 AQL `dialog(...)` 的语义。

---

## 11.3 P0 role 映射表

| AQL Role | UIA | P0 |
|---|---|---|
| `window` | Window | Native |
| `dialog` | Window + IsDialog | Native |
| `pane` | Pane | Native |
| `button` | Button | Native |
| `textbox` | Edit | Native |
| `checkbox` | CheckBox | Native |
| `radio` | RadioButton | Native |
| `combobox` | ComboBox | Native |
| `list` | List | Native |
| `list_item` | ListItem | Native |
| `tree` | Tree | Native |
| `tree_item` | TreeItem | Native |
| `tab` | Tab | Native |
| `tab_item` | TabItem | Native |
| `menu` | Menu | Native |
| `menu_item` | MenuItem | Native |
| `link` | Hyperlink | Native |
| `image` | Image | Native |
| `table` | Table | Native |
| `document` | Document | Native |
| `text` | Text | Native |
| `row` | 无可靠 1:1 ControlType | **先 Unsupported** |
| `cell` | 无可靠 1:1 Cell ControlType | **先 Unsupported / 后续 GridItem hybrid** |

特别是 `row/cell`：

不要为了让 compiler 显示“支持”而粗暴映射成 `DataItem`。

真实语义未验证前，宁可：

```text
Unsupported
```

也不要造成 compiler/executor 语义漂移。

---

# 12. 推荐增加 UIA Property IR

建议：

```rust
pub enum UiaProperty {
    Name,
    AutomationId,
    ClassName,
    Value,
    IsEnabled,
    IsOffscreen,
    HasKeyboardFocus,
    ToggleState,
    IsSelected,
    IsDialog,
}
```

portable 属性映射建议：

| AQL 属性 | UIA 属性 |
|---|---|
| `name` | `UIA_NamePropertyId` |
| `key` | `UIA_AutomationIdPropertyId` |
| `value` | Value Pattern Value property |
| `enabled` | `UIA_IsEnabledPropertyId` |
| `visible` | **反向** `UIA_IsOffscreenPropertyId` |
| `focused` | `UIA_HasKeyboardFocusPropertyId` |
| `checked` | Toggle Pattern ToggleState |
| `selected` | SelectionItem Pattern IsSelected |
| `uia.automation_id` | `UIA_AutomationIdPropertyId` |
| `uia.class_name` | `UIA_ClassNamePropertyId` |

其中 `visible` 是一个典型例子：

```text
AQL:
visible = true
```

不能在 executor 里临时硬编码。

compiler 应直接降成：

```text
IsOffscreen = false
```

这样 explain、cache 和 executor 使用的是同一事实。

---

# 13. Pushdown / Residual 也应改成真正 UIA plan 类型

建议把：

```rust
Vec<PropertyPredicate>
```

替换为类似：

```rust
pub struct UiaNativePredicate {
    pub property: UiaProperty,
    pub operator: UiaNativeOperator,
    pub value: UiaNativeValue,
}
```

以及：

```rust
pub struct UiaResidualPredicate {
    pub property: UiaPropertyProjection,
    pub operator: MatchOperator,
    pub value: PredicateValue,
}
```

最终：

```rust
pub struct UiaMatcherPlan {
    pub role: UiaRoleConstraint,
    pub pushdown: Vec<UiaNativePredicate>,
    pub cache: Vec<UiaPropertyProjection>,
    pub residual: Vec<UiaResidualPredicate>,
}
```

这样 `compile_uia_query()` 才真正完成：

```text
AQL semantics
   ↓
UIA semantics
```

而不是只完成了一半。

---

# 14. Equal / NotEqual 的 native condition

P0 可原生编译：

```text
=
!=
```

`=`：

```text
CreatePropertyCondition
```

`!=`：

```text
CreateNotCondition(
    CreatePropertyCondition(...)
)
```

并与 role condition 使用：

```text
CreateAndCondition
```

组合。

---

# 15. contains / starts_with / ends_with / matches 的 residual

当前 compiler 已把这些放进 residual，这是正确方向。

执行时：

```text
FindAllBuildCache
       ↓
一次性读取 residual 所需属性
       ↓
Rust 本地比较
```

`matches` 正则必须在 query compiler/request 级预编译一次并由所有候选共享，禁止在每个 candidate 上重复 `RegexBuilder::build()`。

不要对每个 candidate 再调用：

```text
get_CurrentName
get_CurrentAutomationId
...
```

否则跨进程 COM round-trip 会快速膨胀。

Microsoft UI Automation 本身提供 `CacheRequest` / `FindAllBuildCache`，当前 compiler 已经把 cache 边界建模出来，因此应该把它真正用起来。

---

# 16. Query Executor 的执行模型

建议定义：

```rust
struct UiaExecutor<'a> {
    automation: &'a IUIAutomation,
}
```

worker 内：

```rust
executor.execute(
    &request.window,
    &request.query_plan,
    &request.action,
)
```

## 16.1 UIA provider timeout 与 ArgusFlow execution budget

worker 初始化必须取得 `IUIAutomation2` 并显式配置：

```text
connection timeout = 2s
transaction timeout = 20s
```

同时每个进入 channel 的请求创建独立 `UiaExecutionBudget`：

```text
deadline = enqueue time + 25s
max_traversal_nodes = 10_000
max_relation_roots = 256
```

deadline 必须包含 worker 排队时间，并在每次 provider 调用前后检查。关系根数按完整请求累计。provider 树必须通过 `RawViewWalker` 增量导航，并在取得每个节点时扣减 `max_traversal_nodes`；禁止先用 subtree `FindAllBuildCache` 物化完整数组、再做后置长度检查。原生 condition 只对单个元素使用 `TreeScope_Element` 求值，因此 provider traversal 在继续导航前已经受硬上限约束。

ArgusFlow 外层异步等待使用同一总时限。单次 timeout 不永久熔断 runtime，而是关闭当前请求入口并启动下一代 MTA worker；health 必须绑定 generation，旧 worker 迟到的退出或失败不得覆盖新 worker 状态。恢复次数有稳定上限，耗尽后才进入持久失败状态。

`Drop` 不得对仍可能卡在第三方 provider 的 worker 执行无上限 `join`。已退出线程可以回收；仍运行线程发送 shutdown 后分离，由配置的 provider timeout 约束最终清理。

---

# 17. 根元素必须来自 prepare 时冻结的 HWND

不要从：

```text
RootElement / Desktop
```

开始全桌面搜索。

执行入口：

```text
PreparedWindowTarget.handle
   ↓
HWND
   ↓
IUIAutomation::ElementFromHandle
```

然后查询限定在该窗口 subtree。

优点：

```text
不会误操作其它进程
性能更稳定
AmbiguousTarget 更可解释
Notepad++ E2E 更稳定
```

## 17.1 Application resource 的 P0 能力边界

当前 `ApplicationSpec` / `AppSession` 只保证 direct-process desktop applications：

```text
absolute executable path
    ↓
Command::spawn PID
    ↓
同一 PID 创建匹配的可见顶层 HWND
```

不覆盖：

```text
bootstrapper -> real app child process
new process -> handoff to existing singleton -> exit
packaged app indirection
```

绝对 EXE 路径也意味着该契约当前是本机绑定，不应宣称工作流天然跨机器可移植。

工作流携带任意绝对 EXE 与参数的信任模型按当前产品决策暂不调整；该已知边界仍需在未来引入不可信工作流来源前重新评审。

复用既有进程时，EXE 身份必须来自文件句柄的卷序列号与文件索引，不能用 `eq_ignore_ascii_case` 比较路径文本。应用资源当前声明 `browser_cdp = false`，因此 `TargetScope::Application + BackendPreference::BrowserCdp` 必须在工作流 validator 阶段直接拒绝。

应用窗口准备分成三个明确步骤：

```text
EnsureRunning
EnsureRestored          required by UIA scope
BestEffortForeground    failure does not block UIA
```

`SetForegroundWindow` 受 Windows foreground-lock 约束，它的失败不能让应用资源获取变成硬失败。只有未来依赖 SendInput 的物理输入计划才声明 `ForegroundRequired`。

---

# 18. HWND 必须带 PID 防止 handle reuse

建议：

```rust
pub struct PreparedWindowTarget {
    pub handle: u64,
    pub process_id: u32,
}
```

execute 前先：

```text
GetWindowThreadProcessId(HWND)
```

重新验证：

```text
HWND 仍有效
PID 仍相同
```

如果窗口已经关闭或 handle 已被系统复用：

```rust
AutomationError::BackendUnavailable {
    backend: BackendKind::WindowsUia,
    ...
}
```

这是运行环境变化，可以让现有 `PreparedPlan` 走 availability fallback。

---

# 19. `Match` 的执行

建议基础流程：

```text
role condition
   +
pushdown condition
   +
ControlViewCondition
        ↓
FindAllBuildCache
        ↓
cached residual filter
        ↓
Vec<ResolvedElement>
```

默认建议基于：

```text
UIA Control View
```

而不是直接扫 Raw View。

P0 的目标是操作用户可交互的标准 UI，不需要把 provider 的所有 raw fragment 都暴露成候选。

---

# 20. `Descendant`

现有：

```rust
UiaPlanExpr::Descendant {
    ancestor,
    target,
}
```

执行语义：

```text
1. 在当前 scope 求 ancestor 结果
2. 对每个 ancestor：
       只在其 TreeScope_Descendants 内执行 target
3. 合并结果
4. 按 UIA tree encounter order 保持稳定顺序
5. runtime id 去重
```

注意：

```text
Descendant
```

必须是**严格后代**，不能再次把 ancestor 自己算进 target。

---

# 21. `Child`

执行语义：

```text
1. 求 parent
2. 对每个 parent：
       只在 TreeScope_Children 中执行 target
3. 合并、去重
```

不要用：

```text
Descendants + 检查 parent
```

代替。

UIA 已经有 `TreeScope_Children`，直接保持语义即可。

---

# 22. `Any`

`Any` 的领域契约是按声明顺序执行的 fallback，而不是结果集合 union：

```text
for branch in executable_branches:
    results = execute branch
    if results is not empty:
        return results

return []
```

第一个非空 branch 如果返回多个元素，仍必须由最终唯一性校验报告：

```text
AmbiguousTarget
```

不能为了 fallback 语义偷偷取第一个元素；只有显式 `first(...)` / `nth(...)` 可以消除歧义。

跨 backend 时，compiler 删除不支持的 branch 还不够。`any` 必须在 compiler 阶段展开为独立 Planner alternative，每个候选只携带一条完整路径：

```text
BranchPath([outer_index, nested_index, ...])
```

同一 backend 支持原始 branch 0 和 branch 2、但不支持 branch 1 时，必须生成两个候选 `[0]` 与 `[2]`；禁止继续把 0 和 2 合并在同一 backend plan 内。关系表达式两侧存在 `any` 时，对两侧替代方案做笛卡尔积并按查询树顺序连接路径。动作能力也逐替代方案计算，后序不支持的角色不能拒绝可执行的前序分支。

Router 排序固定为：

```text
BranchPath lexicographic order
    ↓
SupportLevel
    ↓
ContextFitness
    ↓
QueryCost
    ↓
backend tie-break
```

因此：

```text
any(
    button(dom.test_id = "save"),
    button(uia.automation_id = "save")
)
```

必须先选择能执行 branch 0 的 CDP 候选；不能因为 UIA 的稳定 tie-break 更靠前而直接执行 branch 1。

---

# 23. `First`

```text
first(query)
```

P0 可以先：

```text
execute full query
take first
```

后续再优化成：

```text
FindFirstBuildCache
```

不要为了第一版性能优化改变语义。

---

# 24. `Nth`

AQL 已经定义：

```text
nth(query, 1)
```

为 1-based。

executor 必须保持：

```rust
index - 1
```

不存在该项时：

```text
TargetNotFound
```

而不是：

```text
AmbiguousTarget
```

---

# 25. `Not`

当前 UIA compiler 把 `not(...)` 标记为 Emulated。

建议真实 UIA P0 **不要急着宣称 Ready 支持 `Not`**。

原因是：

```text
not(query)
```

需要明确“补集的 universe 是谁”。

如果直接对整个 window Control View 做：

```text
all elements - query results
```

不仅非常昂贵，而且很容易得到数千个无意义元素。

建议两种选择之一：

### 方案 A（推荐 P0）

暂时让 UIA compiler 对 `Not`：

```text
Unsupported
```

直到 AQL 对 complement scope 有明确执行约束。

### 方案 B（P1）

定义：

```text
当前 relation scope 的 Control View candidate universe
```

再做集合排除，并配置明确的 candidate limit。

不要保持现在：

```text
compiler 显示 Emulated
executor 却没有可证明的执行语义
```

---

# 26. 唯一目标解析必须统一

query executor 最终输出：

```rust
Vec<ResolvedElement>
```

然后统一：

```text
0
  -> AutomationError::TargetNotFound

1
  -> execute action

>1
  -> AutomationError::AmbiguousTarget
```

只有：

```text
first(...)
nth(...)
```

显式消除了歧义后，才允许返回一个元素。

这与当前 `argusflow-core::AutomationError` 完全一致，不需要另造错误协议。

---

# 27. Click：P0 只做真正的 UIA Invoke

`AutomationAction::Click` 在 UIA backend 内建议：

```text
Resolved Element
   ↓
GetCurrentPattern(UIA_InvokePatternId)
   ↓
IUIAutomationInvokePattern::Invoke()
```

这是真正的语义 UIA 点击。

P0 不能再把所有可查询角色统一映射成 Invoke：

| 最终目标角色 | Click 能力 |
|---|---|
| Button / Hyperlink | `Native` |
| MenuItem | `RequiresRuntimePatternCheck` |
| CheckBox / Radio | `Unsupported`，后续分别设计 Toggle / SelectionItem |
| ListItem / TreeItem / TabItem / ComboBox | `Unsupported`，不得错误报告 Invoke Native |

`any(...)` 的任一可执行分支只要可能先返回不支持 Click 的角色，整个 UIA 动作候选就必须拒绝；不能跳过更早 branch 改点后续角色。

P0 **不要**在 UIA backend 里偷偷做：

```text
GetClickablePoint
SetCursorPos
SendInput
```

否则：

```text
Explain 显示 WindowsUia
实际却是物理鼠标点击
```

会破坏 BackendKind 的真实性。

如果目标没有 InvokePattern：

```rust
AutomationError::BackendFailed {
    backend: BackendKind::WindowsUia,
    message: "... does not expose InvokePattern",
}
```

后续再按明确设计增加：

```text
TogglePattern
SelectionItemPattern
ExpandCollapsePattern
LegacyIAccessible DoDefaultAction
```

但不能把这些行为偷偷塞进 Click 而不更新 explain。

---

# 28. SetValue：P0 使用 ValuePattern

`AutomationAction::SetValue`：

```text
Resolved Element
   ↓
GetCurrentPattern(UIA_ValuePatternId)
   ↓
IUIAutomationValuePattern
   ↓
CurrentIsReadOnly
   ↓
SetValue(value)
```

只要：

```text
ValuePattern 存在
IsReadOnly == false
```

就直接设置。

静态动作能力边界：

```text
Edit      -> Native
ComboBox  -> RequiresRuntimePatternCheck
其它角色   -> Unsupported
```

如果没有 ValuePattern：

```text
P0 -> BackendFailed
```

不要使用键盘输入伪装。

---

# 29. 为什么 P0 不要求用 UIA 给 Notepad++ 主编辑器写文本

Notepad++ 的主编辑区域来自：

```text
Scintilla
```

它是自定义 Windows control，不是标准 Win32 Edit。

不同 Notepad++ / Scintilla 版本、渲染方式和 accessibility provider 状态下，它暴露的 UIA tree / TextPattern 能力可能不同。

而 Microsoft UIA 对：

```text
单行 Edit
```

通常适合 `ValuePattern`；

对：

```text
多行 document/editor
```

通常更偏 `TextPattern`，而 TextPattern 本身不是一个通用的客户端“直接 SetValue”接口。

因此 P0 验收不应该绑定：

```text
必须通过 ValuePattern 修改 Scintilla 编辑区
```

否则测试的是 Scintilla provider 差异，而不是 ArgusFlow UIA executor 的正确性。

**P0 用 Notepad++ 的标准菜单、对话框、Button、ComboBox/Edit 做真实 UIA 验收。**

主编辑器 UIA 支持作为 P1 exploratory case。

---

# 30. Notepad++ 测试环境固定方式

建议要求：

```text
Notepad++ 64-bit
英文 UI
无第三方插件
独立进程
不加载历史 session
```

启动参数：

```text
-multiInst
-nosession
-noPlugin
```

可进一步加：

```text
-settingsDir="<temporary test settings dir>"
```

避免用户本机配置污染测试。

建议通过环境变量提供 exe：

```powershell
$env:ARGUSFLOW_NOTEPADPP_EXE="C:\Program Files\Notepad++\notepad++.exe"
```

不要在 Rust 测试里写死：

```text
C:\Program Files\Notepad++\notepad++.exe
```

---

# 31. Notepad++ fixture

建议：

```text
crates/argusflow-windows/tests/support/notepadpp.rs
```

负责：

```text
启动 Notepad++
拿到 PID
等待顶层 HWND
构造 WindowContext
测试结束 Kill/Close 进程
失败时 dump UIA tree
```

启动：

```rust
Command::new(exe)
    .args([
        "-multiInst",
        "-nosession",
        "-noPlugin",
    ])
```

拿 HWND 不应靠窗口标题猜测。

建议：

```text
EnumWindows
  ↓
GetWindowThreadProcessId
  ↓
PID == spawned child PID
  ↓
visible top-level window
```

得到稳定：

```rust
WindowContext {
    handle,
    process_id,
}
```

---

# 32. Notepad++ E2E 不依赖“当前用户前台窗口”

真实 E2E 推荐通过：

```text
StaticExecutionContext
```

注入刚刚启动的 Notepad++：

```rust
ExecutionContext {
    foreground_window: Some(notepadpp_window),
    accessibility: AccessibilityContext { ready: true },
    ...
}
```

然后构造真实：

```text
UiaRuntime
UiaBackend
ActionRouter
PreparedPlan
```

这样测试的是：

```text
ArgusFlow ActionRouter
+
真实 UIA Backend
+
真实 Notepad++ provider
```

但不会因为测试时用户碰了一下鼠标切换前台窗口而随机失败。

另外单独保留一个很小的：

```text
WindowsExecutionContextProvider smoke test
```

测试实际前台窗口读取即可。

---

# 33. 建议的 Notepad++ P0 E2E 用例

测试文件：

```text
crates/argusflow-windows/tests/uia_notepadpp_e2e.rs
```

并：

```rust
#[cfg(windows)]
#[ignore = "requires interactive Windows desktop and Notepad++"]
```

---

## Case 1：真实定位 Notepad++ window

AQL：

```text
window(name contains "Notepad++")
```

验证：

```text
PreparedPlan.selected_backend() == WindowsUia
PlanExplain.availability == Ready
query 返回唯一 Window
```

这是最基础的：

```text
HWND -> ElementFromHandle -> AQL matcher
```

链路测试。

---

## Case 2：InvokePattern 打开 Search menu

英文测试环境下：

```text
window(name contains "Notepad++")
    >> menu_item(name = "Search")
```

执行：

```rust
AutomationAction::Click
```

断言：

```text
ActionOutcome.backend == WindowsUia
```

并验证 Search menu 已展开/其 Find 项可被 UIA 查到。

如果顶层菜单 provider 不暴露 InvokePattern 而暴露 ExpandCollapsePattern，则：

```text
先保留失败诊断
再把 MenuItem Click 的 pattern policy 作为单独 P0.5 设计
```

不要为了“让测试绿”直接改成 SendInput。

---

## Case 3：Invoke Find...

在 Search menu 已打开后定位：

```text
menu_item(name contains "Find")
```

更严格时可以使用：

```text
first(menu_item(name starts_with "Find"))
```

执行 Click。

然后查询：

```text
dialog(name contains "Find")
```

应得到唯一真实对话框。

---

## Case 4：SetValue 写入 Find what

Notepad++ Find 对话框中的输入控件在不同 provider 版本下可能呈现为：

```text
Edit
```

或：

```text
ComboBox + Edit
```

因此建议用 AQL 的 `any(...)`：

```text
dialog(name contains "Find")
    >> first(
        any(
            textbox(name contains "Find what"),
            combobox(name contains "Find what")
        )
    )
```

执行：

```rust
AutomationAction::SetValue {
    value: "argusflow-uia-e2e"
}
```

然后测试 helper 通过真实 UIA ValuePattern/current value readback：

```text
argusflow-uia-e2e
```

注意：

```text
readback helper
```

可以是 `pub(crate)` / test support，不需要为了测试给 `AutomationAction` 新增 GetValue 动作。

---

## Case 5：关闭 Find dialog

优先：

```text
dialog(name contains "Find")
    >> button(name = "Close")
```

通过：

```text
InvokePattern
```

关闭。

断言：

```text
dialog(name contains "Find")
```

返回：

```text
TargetNotFound
```

---

## Case 6：AmbiguousTarget

在 Find dialog 内执行故意不唯一的：

```text
button()
```

如果返回多个 button：

```text
AutomationError::AmbiguousTarget
```

并断言：

```text
matches > 1
```

验证 executor 没有偷偷“拿第一个”。

---

## Case 7：TargetNotFound

例如：

```text
button(name = "__argusflow_uia_missing_target__")
```

应返回：

```text
AutomationError::TargetNotFound
```

而不是：

```text
BackendUnavailable
```

否则 Router 会错误认为 UIA 环境坏了。

---

## Case 8：Residual Regex

打开 Find dialog 后：

```text
button(name matches /Find|Close/i)
```

要求 explain 中出现：

```text
ResidualFilter
CacheRequest
```

并真实经过：

```text
FindAllBuildCache
+
Rust regex filter
```

用于验证当前 compiler 已存在的：

```text
pushdown / cache / residual
```

不是纸面 plan。

---

# 34. E2E 失败时必须 dump UIA tree

Notepad++ provider 可能随着版本更新变化。

所以 test fixture 应提供：

```rust
dump_control_view(
    root,
    max_depth: 6,
)
```

只在测试失败时输出，例如：

```text
Window
  name="new 1 - Notepad++"
  class="Notepad++"

  MenuBar
    MenuItem name="File"
    MenuItem name="Edit"
    MenuItem name="Search"

  Pane
  ...

  Window
    name="Find"
    is_dialog=true
    ...
```

每个节点至少输出：

```text
ControlType
Name
AutomationId
ClassName
IsEnabled
IsOffscreen
```

这样 Notepad++ 升级后 locator 变化能快速诊断。

不要默认做完整 snapshot golden test；UIA tree 太容易受 Windows / Notepad++ 版本影响。

---

# 35. 测试配置不要假设中文或用户本机语言

P0 的 Notepad++ E2E 应明确声明：

```text
英文 UI 是测试 fixture 的组成部分
```

因为 AQL：

```text
menu_item(name = "Search")
```

本身就是按 Accessible Name 测试。

后续可以再加国际化 case：

```text
any(
    menu_item(name = "Search"),
    menu_item(name = "搜索")
)
```

但不要让第一版真实 UIA 验收被本机语言环境绑死。

---

# 36. 推荐的 UIA 内部错误模型

不要直接到处把 `windows::core::Error` 格式化成 String。

建议内部：

```rust
pub(crate) enum UiaError {
    RuntimeInitializationFailed { ... },
    WorkerStopped,
    WindowUnavailable,
    ElementUnavailable,
    NativeCallFailed {
        operation: UiaOperation,
        source: windows::core::Error,
    },
    PropertyTypeMismatch {
        property: UiaProperty,
    },
    RequiredPatternUnavailable {
        pattern: UiaPattern,
    },
    ReadOnlyValue,
}
```

其中：

```rust
pub enum UiaOperation {
    ElementFromHandle,
    CreateCondition,
    BuildCache,
    FindAll,
    ReadProperty,
    GetPattern,
    Invoke,
    SetValue,
}
```

最终集中映射到 `AutomationError`。

分类必须优先看 `HRESULT`，`UiaOperation` 只用于诊断上下文。不能因为错误发生在 FindAll/CreateCondition 就默认归为 BackendUnavailable。

---

# 37. `UiaError -> AutomationError` 映射

建议：

| 情况 | AutomationError |
|---|---|
| UIA worker 未启动/崩溃 | `BackendUnavailable` |
| prepare 冻结的 window 已关闭 | `BackendUnavailable` |
| `UIA_E_TIMEOUT`、RPC timeout/disconnected/server died | `BackendUnavailable` |
| `UIA_E_ELEMENTNOTAVAILABLE` | 同一冻结计划 stale retry 一次，仍失败则 `BackendUnavailable` |
| query 结果 0 | `TargetNotFound` |
| query 结果 >1 | `AmbiguousTarget` |
| `UIA_E_NOTSUPPORTED` / `UIA_E_INVALIDOPERATION` | `BackendFailed`；仅 GetPattern 时转换为 pattern 缺失 |
| `E_INVALIDARG`、属性类型不一致、native IR 不一致 | `BackendFailed` |
| candidate/relation budget 超限 | `BackendFailed` |
| 找到元素但没有 InvokePattern | `BackendFailed` |
| 找到元素但没有 ValuePattern | `BackendFailed` |
| ValuePattern read-only | `BackendFailed` |
| 其它未知 HRESULT | `BackendFailed`，不得静默 fallback |

这个分类非常重要，因为当前 `PreparedPlan`：

```text
BackendUnavailable 允许同一分支/后续候选 fallback
TargetNotFound 只允许推进到更晚的 any branch index
其它错误立即终止
```

所以不能把：

```text
TargetNotFound
```

错误包装成：

```text
BackendUnavailable
```

同样不能把确定性的 compiler/native IR bug 包装成 `BackendUnavailable`，否则未来接入 Vision/OCR 后会由 fallback 掩盖实现错误。

---

# 38. stale element 的处理

UIA 元素在 root、query 和 action 任一阶段都可能失效。

P0 可以做一次非常有限的：

```text
root/query/resolve
  ↓
action
  ↓
UIA element unavailable
  ↓
用同一个冻结 UiaQueryPlan 重新 resolve 一次
  ↓
重试 action 一次
```

这不是 re-plan，因为：

```text
AQL
UiaQueryPlan
HWND
Action
```

都没有改变。

只是在真实 UI tree 更新后重新 materialize frozen plan。

最多一次，避免隐式无限重试。

---

# 39. CacheRequest 设计

每个 `UiaMatcherPlan` 的 cache 应精确来自 compiler 的 residual projection。

例如：

```text
button(
    name matches /save/i,
    enabled = true
)
```

应形成：

```text
ControlType = Button          native
IsEnabled = true             native
Name                         cache
regex(Name)                  residual
```

而不是每次默认缓存几十个 UIA property。

---

# 40. Action pattern 不要塞进 query cache

query cache 负责：

```text
定位
residual
```

最终选中唯一元素后，再读取：

```text
InvokePattern
ValuePattern
```

执行动作。

原因：

```text
pattern availability 是最终元素动作能力
query cache 是 candidate selection snapshot
```

两个职责不同。

P0 先保持简单边界。

---

# 41. 性能边界

真实 UIA 的主要性能成本通常不是 Rust 运算，而是：

```text
跨进程 UIA property round-trip
过大的 tree traversal
重复 CurrentProperty 调用
```

因此 P0 的性能原则：

```text
HWND scoped root
Control View
native condition 尽量 pushdown
residual 属性一次 BuildCache
不逐 candidate 多次跨进程读属性
```

---

# 42. 不建议 P0 做的事情

本次真实接 UIA 不要顺手扩成一个巨型 Windows 自动化框架。

P0 不做：

```text
UIA events subscription
全局 desktop inspector
OCR/UIA 混合 resolver
复杂 spatial query
TextPattern 全功能编辑
Grid/Table 高级导航
ScrollPattern
Drag/Drop
窗口移动缩放
自动 SendInput fallback
UIA Remote Operations
跨 session / service desktop automation
```

先把现有 AQL + Click + SetValue 跑通真实 provider。

---

# 43. 建议修改 `UiaQueryPlan`

推荐目标：

```rust
pub struct UiaQueryPlan {
    pub expression: UiaPlanExpr,
    pub capability: BackendQueryCapability,
    pub normalized: UiQuery,
    pub diagnostics: Vec<Diagnostic>,
}
```

保留外形。

但 `UiaPlanExpr::Match` 内部改为：

```rust
pub struct UiaMatcherPlan {
    pub role: UiaRoleConstraint,
    pub pushdown: Vec<UiaNativePredicate>,
    pub cache: Vec<UiaPropertyProjection>,
    pub residual: Vec<UiaResidualPredicate>,
}
```

这样对上层 API 影响很小。

---

# 44. `compiler.rs` 要增加的核心函数

建议拆：

```rust
fn compile_role(
    role: ElementRole,
) -> Result<UiaRoleConstraint, UiaQueryCompileError>;
```

```rust
fn compile_predicate(
    predicate: &PropertyPredicate,
) -> Result<UiaPredicateCompilation, UiaQueryCompileError>;
```

```rust
enum UiaPredicateCompilation {
    Pushdown(UiaNativePredicate),
    Residual {
        projection: UiaPropertyProjection,
        predicate: UiaResidualPredicate,
    },
}
```

这样：

```text
Role 支持
Property 支持
Native/Residual
Value transform
```

都在 compiler 被证明。

---

# 45. compiler 单元测试要补的内容

当前已有：

```text
compiler_pushes_native_predicates_and_caches_residual_attributes
compiler_preserves_descendant_scope
compiler_rejects_dom_specific_query
compiler_keeps_supported_branch_of_cross_backend_any
```

建议新增：

```text
dialog_compiles_to_window_and_is_dialog
visible_true_compiles_to_is_offscreen_false
key_compiles_to_automation_id
uia_class_name_compiles_native
row_is_not_falsely_reported_native
cell_is_not_falsely_reported_native
not_is_rejected_until_executor_semantics_are_defined
```

如果决定 P0 实现 `Not`，最后一项替换为对应执行语义测试。

---

# 46. executor 单元测试不要依赖真实 Windows provider

真实 Notepad++ E2E 之外，executor 的集合语义最好可以用纯 Rust helper 测：

```text
dedupe
first
nth
ambiguous
any first-non-empty fallback
residual string compare
visible inversion
toggle bool compare
```

UIA COM object 相关逻辑单独保留在 Windows integration tests。

---

# 47. Notepad++ E2E 的建议代码组织

```text
crates/argusflow-windows/tests/
├─ uia_query_compiler.rs
├─ uia_notepadpp_e2e.rs
└─ support/
   ├─ mod.rs
   ├─ notepadpp.rs
   └─ uia_dump.rs
```

测试 helper 不进入 public API。

---

# 48. 推荐 E2E 测试入口

PowerShell：

```powershell
$env:ARGUSFLOW_NOTEPADPP_EXE="C:\Program Files\Notepad++\notepad++.exe"

cargo test `
  -p argusflow-windows `
  --test uia_notepadpp_e2e `
  -- `
  --ignored `
  --nocapture
```

普通单元/编译器测试：

```powershell
cargo test -p argusflow-windows
```

本方案只给出命令，不建议实现代码时自动下载或自动安装 Notepad++。

持续验证分成两层：

```text
.github/workflows/ci.yml
    windows-latest
    cargo test --workspace
    compiler/unit/router/runtime tests

.github/workflows/windows-uia-e2e.yml
    self-hosted + Windows + X64 + interactive
    nightly/manual
    ignored real Notepad++ provider tests
```

交互 runner 必须运行在真实用户 desktop session，不能使用无桌面的 service session。除查询/pattern E2E 外，至少保留一条：

```text
WorkflowDefinition
  -> WorkflowEngine
  -> Application resource
  -> launch Notepad++
  -> WindowsUia action
  -> observable UI state assertion
```

---

# 49. E2E 验收必须证明“真的用了 UIA”

每个成功 Action 至少断言：

```rust
assert_eq!(
    outcome.backend,
    BackendKind::WindowsUia
);
```

测试日志还应能看到：

```text
UIA worker ready
root hwnd=...
query plan=...
native conditions=...
cache properties=...
residual predicates=...
resolved candidates=1
action pattern=Invoke / Value
```

测试中禁止：

```text
SendInput
mouse_event
SetCursorPos
keyboard typing
coordinate locator
```

否则不算 UIA E2E 通过。

---

# 50. Planner Explain 验收

接入前：

```text
WindowsUia
Support: Native
Availability: NotImplemented
```

接入后，Notepad++ E2E 中必须变成：

```text
WindowsUia
Support: Native / Hybrid
Availability: Ready
ContextFitness: Good
```

对于 regex query：

```text
CandidateSource: UIA Button / MenuItem / ...
Pushdown: ...
CacheRequest: Name
Residual: regex
Action: InvokePattern / ValuePattern
```

建议 `explain_uia_plan()` 最后再由 action prepare 追加：

```text
PlanStepKind::Action
```

例如：

```text
InvokePattern
```

或：

```text
ValuePattern::SetValue
```

这样 UI Explain 不只解释“怎么找”，还解释“怎么操作”。

---

# 51. `Action` 是否支持也要进入 prepare explain

当前 query compiler 只证明：

```text
这个元素能找到
```

它无法仅凭 role 永远证明：

```text
目标实例一定有 InvokePattern
```

所以 P0 必须采用查询与动作联合计划：

```text
prepare:
    query semantics supported
    collect final target roles
    derive UiaActionSupport
    combine query capability + action capability

execute:
    resolve element
    check actual pattern
```

在 plan 中增加：

```rust
pub enum UiaActionPlan {
    Invoke,
    SetValue { value: String },
}

pub enum UiaActionSupport {
    Native,
    RequiresRuntimePatternCheck,
    Unsupported,
}

pub struct UiaPreparedPlan {
    pub query: UiaQueryPlan,
    pub action: UiaActionPlan,
    pub action_support: UiaActionSupport,
    pub capability: BackendQueryCapability,
}
```

`PlanExplain.support` 必须读取 `UiaPreparedPlan.capability`，而不是继续直接读取 `query_plan.capability`。当动作需要实例 pattern 复验时，联合支持至少是 `Hybrid`；`Unsupported` 在 prepare 阶段直接拒绝。

`UiaPreparedExecution` 保存：

```text
UiaPreparedPlan
```

这样 action 也被冻结，不需要 execute 时重新 `match AutomationAction` 做规划。

---

# 52. 推荐把 Action 降成 `UiaActionPlan`

例如：

```rust
pub enum UiaActionPlan {
    Invoke,
    SetValue {
        value: String,
    },
}
```

prepare：

```text
AutomationAction::Click
    -> UiaActionPlan::Invoke

AutomationAction::SetValue
    -> UiaActionPlan::SetValue
```

executor：

```text
ResolvedElement
   +
UiaActionPlan
   ↓
action.rs
```

这进一步符合：

```text
Prepared Plan 是执行事实来源
```

---

# 53. `AutomationAction` 不需要改协议

P0 不需要修改：

```rust
pub enum AutomationAction {
    Click { ... },
    SetValue { ... },
}
```

也不需要增加：

```text
UiaClick
UiaSetValue
```

AQL / Action 保持跨 backend。

UIA 特定的 pattern 只存在于：

```text
argusflow-windows
```

内部 plan。

---

# 54. 前端不需要新增“UIA 模式”

当前 docs 已明确：

```text
UI 不主导 Planner
```

所以真实 UIA 接入后，不建议在普通 Action 节点新增：

```text
使用 UIA
使用 InvokePattern
使用 ValuePattern
```

这种开关。

用户仍然写：

```text
button(name = "Close")
```

或：

```text
textbox(name contains "Find what")
```

Router 自动选择真实 UIA。

只有 Debug / Explain 中显示：

```text
Selected Backend: WindowsUia
Action: InvokePattern
```

---

# 55. Notepad++ 用例为什么适合当前阶段

Notepad++ 能覆盖：

```text
Win32 top-level Window
native Menu/MenuItem
Dialog
Button
ComboBox/Edit
自定义 Scintilla control
```

所以它非常适合作为“第一真实 UIA 应用”：

```text
标准控件能验证基础 UIA executor
Scintilla 能暴露未来复杂 provider 差异
```

但 P0 要先以标准控件为硬验收，Scintilla 作为探索性测试。

---

# 56. P1：Notepad++ Scintilla 探索测试

可新增：

```text
uia_notepadpp_scintilla_probe.rs
```

只做：

```text
dump editor subtree
ControlType
ClassName
supported pattern ids
Name
AutomationId
```

如果当前目标版本暴露：

```text
Document + TextPattern
```

再补：

```text
document()
```

查询和文本读取。

不要反过来为了 Notepad++ Scintilla 特例破坏 AQL portable semantics。

---

# 57. P1：SetValue fallback

如果后续确实需要支持：

```text
Document / rich editor
```

建议按明确策略扩展：

```text
ValuePattern
   ↓ unavailable
LegacyIAccessiblePattern::SetValue
   ↓ unavailable
BackendFailed
```

是否再进入：

```text
SendInput
```

必须由更高层明确的 action fallback 设计决定。

**不要直接在 UIA backend 内嵌 SendInput。**

---

# 58. 安全与线程规则

所有 `unsafe` 只包最小 Win32/COM 调用，例如：

```text
CoInitializeEx
CoCreateInstance
CoUninitialize
HWND conversion
GetWindowThreadProcessId
```

不要把整个 executor 标成：

```rust
unsafe fn execute(...)
```

向其它模块暴露安全 Rust API。

---

# 59. Runtime 生命周期

建议：

```text
AppState
  owns Arc<UiaRuntime>
      ↓
Uia worker lives for application lifetime
      ↓
AppState drop
      ↓
close request channel
      ↓
worker exits
      ↓
CoUninitialize on same worker thread
```

避免 global singleton。

---

# 60. Worker 崩溃/退出

`UiaRuntimeHealth` 应能区分：

```text
Ready
Stopped
InitializationFailed
```

channel send 失败：

```text
BackendUnavailable
```

不要 panic。

---

# 61. 第一阶段代码改动清单

## Step 1：把 UIA logical plan 降成真正 native plan

修改：

```text
uia/plan.rs
uia/compiler.rs
uia/explain.rs
```

新增：

```text
uia/native.rs
```

完成：

```text
role mapping
property mapping
visible inversion
dialog mapping
row/cell capability 修正
```

---

## Step 2：实现 UIA runtime worker

新增：

```text
uia/runtime.rs
uia/error.rs
```

完成：

```text
COM apartment
CUIAutomation8
typed request
oneshot result
health
shutdown
```

---

## Step 3：实现 native condition/cache

新增：

```text
uia/condition.rs
uia/cache.rs
uia/property.rs
```

完成：

```text
PropertyCondition
NotCondition
AndCondition
ControlViewCondition
CacheRequest
cached property projection
residual evaluation
```

---

## Step 4：实现 query executor

新增：

```text
uia/executor.rs
```

完成：

```text
ElementFromHandle
Match
Descendant
Child
Any
First
Nth
dedupe
TargetNotFound
AmbiguousTarget
```

`Not` 按前述决定：

```text
P0 Unsupported
```

或完整实现后再开放。

---

## Step 5：实现 action

新增：

```text
uia/action.rs
```

完成：

```text
Click -> InvokePattern
SetValue -> ValuePattern
```

---

## Step 6：接入 Backend

拆当前：

```text
uia/mod.rs
```

到：

```text
uia/backend.rs
```

把：

```text
RuntimeAvailability::NotImplemented
```

替换为真实 availability。

把 placeholder `execute()` 替换成：

```text
runtime.execute(request).await
```

---

## Step 7：接入 ExecutionContext

修改：

```text
crates/argusflow-windows/src/context.rs
```

让：

```text
accessibility.ready
```

反映 UIA runtime health。

---

## Step 8：接入 Tauri composition root

修改：

```text
src-tauri/src/runtime.rs
```

共享同一个：

```text
Arc<UiaRuntime>
```

给：

```text
UiaBackend
WindowsExecutionContextProvider
```

---

## Step 9：补 Notepad++ E2E

新增：

```text
tests/uia_notepadpp_e2e.rs
tests/support/notepadpp.rs
tests/support/uia_dump.rs
```

完成本文 Case 1~8。

---

# 62. 建议的完成定义（Definition of Done）

真实 UIA 第一阶段完成必须同时满足：

- [ ] `UiaBackend` 不再是 unit struct，而是绑定真实 `UiaRuntime`
- [ ] UIA COM client 在专用 worker thread 初始化和使用
- [ ] `IUIAutomation2` 显式配置 connection/transaction timeout
- [ ] 请求 deadline、硬 traversal node budget、relation root budget 生效
- [ ] 单次 timeout 触发有上限的 generation recovery，而不是永久熔断 runtime
- [ ] COM interface 不跨 Tokio worker thread 传播
- [ ] `RuntimeAvailability::NotImplemented` 从真实 UIA candidate 消失
- [ ] `AccessibilityContext.ready` 能反映 UIA runtime 状态
- [ ] `ElementFromHandle` 使用 prepare 冻结的 Notepad++ HWND
- [ ] `Match` 能执行 ControlType + PropertyCondition
- [ ] residual query 使用 `CacheRequest`
- [ ] `Descendant` 使用严格 descendants scope
- [ ] `Child` 使用 children scope
- [ ] `Any` 按声明顺序返回第一个非空 branch
- [ ] Backend compiler 为每个 `BranchPath` 生成独立 candidate 并按字典序全局排序
- [ ] `First` / `Nth` 正确选择
- [ ] 未显式选择时多个元素返回 `AmbiguousTarget`
- [ ] 无元素返回 `TargetNotFound`
- [ ] Click 使用真实 `InvokePattern`
- [ ] SetValue 使用真实 `ValuePattern`
- [ ] Query capability 与 Action capability 联合生成 Explain support
- [ ] Checkbox/Radio 等非 Invoke 角色不会错误报告 Click Native
- [ ] UIA backend 内没有 SendInput fallback
- [ ] `dialog` 不再被粗暴等价成普通 Window
- [ ] `row/cell` 未验证前不再错误宣称 Native
- [ ] Notepad++ Window 查询通过
- [ ] Notepad++ Search/Find UIA Invoke 通过
- [ ] Notepad++ Find dialog `SetValue` 通过
- [ ] `ActionOutcome.backend == WindowsUia`
- [ ] Planner Explain 中 availability 为 Ready
- [ ] Notepad++ E2E 失败时输出有限深度 UIA tree
- [ ] HRESULT 分类只允许 provider/window/timeout 错误触发 fallback
- [ ] 普通 Windows CI 持续运行 compiler/unit/router/runtime tests
- [ ] 交互式 Windows runner 持续运行真实 Notepad++ UIA E2E
- [ ] WorkflowEngine -> Application resource -> UIA 有可观察 UI 状态断言

---

# 63. 推荐的首个可交付范围

如果希望第一版尽快形成闭环，建议把范围卡死为：

```text
Query:
    Match
    Descendant
    Child
    Any
    First
    Nth

Predicate:
    =
    !=
    contains
    starts_with
    ends_with
    matches

Role:
    Notepad++ P0 实际用到的
    Window
    Dialog
    MenuItem
    Button
    TextBox
    ComboBox
    Pane

Action:
    Click -> Invoke
    SetValue -> Value

Root:
    prepared HWND

Test:
    Notepad++ real E2E
```

先不要因为：

```text
Row
Cell
Not
Scintilla TextPattern
Scroll
Toggle
Selection
```

拖延真实 UIA 主链路。

---

# 64. 一条完整的预期执行链路

以 Notepad++ Find dialog 为例。

用户 Action：

```text
target:
    dialog(name contains "Find")
        >> first(
            any(
                textbox(name contains "Find what"),
                combobox(name contains "Find what")
            )
        )

action:
    SetValue("argusflow-uia-e2e")
```

运行：

```text
WorkflowEngine
  ↓
ActionRouter
  ↓
UiaBackend::prepare
  ↓
parse_stored_query
  ↓
compile_uia_query
  ↓
UiaQueryPlan
  ↓
availability = Ready
  ↓
PreparedCandidate
  ↓
PreparedPlan selects WindowsUia
  ↓
UiaPreparedExecution::execute
  ↓
UiaRuntime request
  ↓
argusflow-uia worker
  ↓
ElementFromHandle(Notepad++ HWND)
  ↓
Find dialog
  ↓
FindAllBuildCache
  ↓
residual filter
  ↓
first(any(...))
  ↓
唯一 Edit/ComboBox target
  ↓
ValuePattern::SetValue
  ↓
ActionOutcome {
    backend: WindowsUia,
    ...
}
```

这条链路中：

```text
没有重新 parse
没有重新 plan
没有 UI 决策 backend
没有坐标
没有鼠标
没有键盘
没有 SendInput
```

这才是当前 ArgusFlow 架构下“对接真实 UIA”的正确落地形态。

---

# 65. 参考：当前项目文件

- `docs/架构.md`  
  https://github.com/SLOE-debug/argusflow/blob/main/docs/%E6%9E%B6%E6%9E%84.md

- `docs/ArgusFlow_AQL_统一UI查询语言设计方案.md`  
  https://github.com/SLOE-debug/argusflow/blob/main/docs/ArgusFlow_AQL_%E7%BB%9F%E4%B8%80UI%E6%9F%A5%E8%AF%A2%E8%AF%AD%E8%A8%80%E8%AE%BE%E8%AE%A1%E6%96%B9%E6%A1%88.md

- `docs/ArgusFlow AQL 审计与重构方案.md`  
  https://github.com/SLOE-debug/argusflow/blob/main/docs/ArgusFlow%20AQL%20%E5%AE%A1%E8%AE%A1%E4%B8%8E%E9%87%8D%E6%9E%84%E6%96%B9%E6%A1%88.md

- `crates/argusflow-windows/src/uia/mod.rs`  
  https://github.com/SLOE-debug/argusflow/blob/main/crates/argusflow-windows/src/uia/mod.rs

- `crates/argusflow-windows/src/uia/compiler.rs`  
  https://github.com/SLOE-debug/argusflow/blob/main/crates/argusflow-windows/src/uia/compiler.rs

- `crates/argusflow-windows/src/uia/plan.rs`  
  https://github.com/SLOE-debug/argusflow/blob/main/crates/argusflow-windows/src/uia/plan.rs

- `crates/argusflow-agent/src/plan.rs`  
  https://github.com/SLOE-debug/argusflow/blob/main/crates/argusflow-agent/src/plan.rs

- `crates/argusflow-agent/src/context.rs`  
  https://github.com/SLOE-debug/argusflow/blob/main/crates/argusflow-agent/src/context.rs

- `crates/argusflow-core/src/automation.rs`  
  https://github.com/SLOE-debug/argusflow/blob/main/crates/argusflow-core/src/automation.rs

- `crates/argusflow-core/src/error.rs`  
  https://github.com/SLOE-debug/argusflow/blob/main/crates/argusflow-core/src/error.rs

- `src-tauri/src/runtime.rs`  
  https://github.com/SLOE-debug/argusflow/blob/main/src-tauri/src/runtime.rs

- `AGENTS.md`  
  https://github.com/SLOE-debug/argusflow/blob/main/AGENTS.md

---

# 66. 参考：Microsoft UI Automation

- UI Automation Control Patterns Overview  
  https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-controlpatternsoverview

- Caching UI Automation Properties and Control Patterns  
  https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-cachingforclients

- `IUIAutomationElement::FindAllBuildCache`  
  https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-findallbuildcache

- UI Automation Control Type Identifiers  
  https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-controltype-ids

- UI Automation Automation Element Property Identifiers  
  https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-automation-element-propids

---

# 67. 参考：Notepad++ / Scintilla

- Notepad++ Command Line Arguments  
  https://github.com/notepad-plus-plus/npp-usermanual/blob/master/content/docs/command-prompt.md

- Scintilla Documentation  
  https://www.scintilla.org/ScintillaDoc.html

Notepad++ E2E 的目的不是依赖其内部 API，而是把它当成一个真实 Win32/UIA provider 宿主，验证 ArgusFlow 的完整 UIA client 链路。

---

# 68. 最终建议

当前项目最合适的落地顺序不是：

```text
先继续扩 AQL
先做 UIA Inspector
先接 Scintilla
先加 SendInput fallback
```

而是：

```text
1. compiler 降成真正 UIA-native plan
2. UIA 专用 COM worker
3. HWND scoped query executor
4. CacheRequest + residual
5. InvokePattern / ValuePattern
6. availability/context 接线
7. Notepad++ 标准控件 E2E
8. 再探索 Scintilla / TextPattern
```

这样每一步都沿用当前 ArgusFlow 已建立的：

```text
AQL
Backend Compiler
ExecutionContext
PreparedPlan
Plan Explain
ActionRouter
```

边界，不会在“终于接上真实 UIA”时把刚完成的 AQL/Planner 重构重新破坏掉。
