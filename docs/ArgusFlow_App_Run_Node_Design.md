# ArgusFlow「App Run / 数字员工 Workflow Node」架构方案

> 仓库：`SLOE-debug/argusflow`
> 分析基线：`main`（2026-08-25 读取）
> 目标：Windows-only、高性能、固定流程为主的数字员工 / RPA Workflow，优先通过 UIA、CDP、Vision/OCR 等快速路径操作应用，同时允许 CLI / PowerShell 等系统级能力。
> 本文重点回答：**App Run 应该暴露什么 Node？是否应该做一个万能 Run Node？CLI/PowerShell 是否应该混进通用 Node？**

---

## 0. 结论先行

我的建议不是：

```text
Run Node
  ├─ UIA
  ├─ CDP
  ├─ OCR
  ├─ CLI
  └─ PowerShell
```

也不是：

```text
一个 Universal Node
  mode = UIA / CDP / OCR / CLI / PS / ...
```

而是把 ArgusFlow 的工作流模型正式拆成三类东西：

```text
┌────────────────────────────────────────────────────┐
│                  Workflow / Control                │
│ Start / End / If / Loop / Retry / Wait / Subflow   │
└────────────────────────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────┐
│                   Resource / Scope                 │
│ App Session / Browser Session / File / Credential  │
└────────────────────────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────┐
│                 Semantic Operations                │
│ UI Action / Read / Command / Transform / Assert    │
└────────────────────────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────┐
│               Backend Planner / Executor           │
│ UIA → CDP → VisualCache → OCR → Grounding → Input  │
└────────────────────────────────────────────────────┘
```

### 核心推荐

**1. App 不应该叫 `Run`，应该是一种可复用的 `AppSession` / `Application Resource`。**

面向用户的节点可以叫：

```text
应用
打开或连接应用
```

内部语义叫：

```rust
AcquireApplication
EnsureApplication
AppSession
```

默认是 **幂等的 Ensure / Attach-or-Start**，而不是“无脑 spawn 一次”。

---

**2. UIA / CDP / OCR / Grounding / SendInput 不应该成为用户选择的 Node 类型。**

它们应该继续留在你现在已经做对的：

```text
AutomationAction
    ↓
ActionRouter
    ↓
PreparedCandidate
    ↓
PreparedPlan
    ↓
Backend executor
```

里面。

用户表达的是：

```text
点击“导出”
读取订单号
填写客户名称
```

而不是：

```text
执行 UIA FindFirst
执行 CDP DOM.querySelector
执行 OCR 点击
```

---

**3. CLI / PowerShell 不应该塞进 UI Action 的 Backend Router。**

因为：

```text
UIA / CDP / OCR
```

是在实现**同一个 UI 语义动作**的不同执行路径。

而：

```text
PowerShell / CLI
```

本质上是另一种 Operation：

```text
启动进程
执行命令
获得 stdout/stderr/exit code
```

它不是“Click 的另一个 backend”。

因此应该有独立的：

```text
Command Node
```

或者内部：

```rust
Operation::Command(...)
```

---

**4. 真正需要优先补的，不只是 App Node，而是 Runtime 的“数据面 + 资源面”。**

你的典型场景：

```text
打开软件 A
读取信息
打开软件 B
填写表单
```

意味着工作流必须支持：

```text
App A Session
        │
        ▼
Read Node ─────► output.value
                    │
                    ▼
App B Session ─► SetValue(value = NodeOutputRef)
```

当前 ArgusFlow 的 `workflow.variables` 是只读 JSON，`ActionOutcome` 也只有：

```text
backend
message
```

还没有真正的一等公民：

```text
NodeOutput
ValueRef
ResourceRef
ExecutionState
```

所以从产品上看你在问“App Run Node 怎么设计”，从 Runtime 上看真正的问题其实是：

> **ArgusFlow 要从“控制流 DAG + Action”升级成“控制流 + 数据流 + 资源生命周期”的 Workflow Runtime。**

---

# 1. 当前 ArgusFlow 已经做对了什么

目前代码的总体方向其实非常适合继续往数字员工演进。

## 1.1 Workflow 层目前很干净

当前：

```rust
pub enum WorkflowNodeKind {
    Start,
    Log { ... },
    Delay { ... },
    Condition { ... },
    Action { action: AutomationAction },
    End,
}
```

这个设计的好处是：

- Workflow Engine 不知道 UIA 细节
- Workflow Engine 不知道 CDP 细节
- UI Action 被统一收敛到 `AutomationAction`
- 控制流与执行后端分离

这个边界应该保留。

---

## 1.2 ActionRouter / PreparedPlan 是正确方向

现在已经有：

```text
ActionBackend::prepare(
    action,
    ExecutionContext
)
    ↓
PreparedCandidate
    ↓
ActionRouter 排序
    ↓
PreparedPlan
    ↓
execute()
```

并且已经明确了：

```text
Semantic Support
Runtime Availability
Context Fitness
Cost
```

这些信息分开。

这是很好的基础。

以后即使加：

```text
Native API
Win32 message
Office COM
SAP GUI API
自定义插件 Driver
```

也可以继续沿用这个思想。

---

## 1.3 你现在实际上已经实现了“隐式 App Run”

当前：

```rust
TargetLocator::ApplicationQuery {
    application: ApplicationTarget,
    query: AqlQuery,
}
```

UIA Backend 会在 Action 执行前：

```text
寻找已经存在的同 EXE 窗口
        ↓
如果存在：复用
        ↓
恢复最小化窗口
        ↓
best-effort foreground
        ↓
如果不存在：启动 EXE
        ↓
等待匹配窗口
        ↓
UIA Query
        ↓
Action
```

而 `WindowService::resolve_application()` 已经实现：

```text
attach existing
or
spawn process
wait window
restore
foreground
```

也就是说：

> **ArgusFlow 现在并不缺“启动 App 的能力”，缺的是把这个能力提升成 Workflow 级 Resource。**

当前 `ApplicationQuery` 是一个非常不错的 P0 / MVP 设计。

但是当 Workflow 进入：

```text
App A
  ↓
连续执行 20 个动作
  ↓
App B
  ↓
执行 10 个动作
  ↓
回到 App A
```

以后，`ApplicationTarget` 每个 Action 都重复携带就会变得不自然。

---

# 2. 为什么我不建议暴露一个普通 `Run App` Node

最直接的设计可能是：

```text
[Run App]
exe = ...
args = ...
```

然后：

```text
Run App
   ↓
Click
   ↓
Read
```

这比当前 action 内嵌 application 已经更进一步，但仍然有一个核心问题：

> `Run` 描述的是“发生一次进程启动行为”，而 Workflow 真正需要的是“从这里开始，我拥有一个可用的应用会话”。

两者不是一回事。

---

## 2.1 `Run` 是命令语义

`Run App` 很容易被理解成：

```text
CreateProcess()
```

那遇到：

- App 已经运行
- App 单例
- App 有多个实例
- App 已最小化
- App 启动器会拉起另一个进程
- App 需要恢复窗口
- App 启动后要等待 ready
- App 已崩溃，需要重新获取
- Workflow 是 retry

就会越来越麻烦。

---

## 2.2 数字员工真正需要的是“资源获取”

更准确的模型应该是：

```text
Acquire / Ensure Application
```

语义：

```text
我要一个满足这个 Profile 的 App Session。

如果已经存在：
    attach

如果没有：
    start

如果最小化：
    restore

如果尚未 ready：
    wait

最后：
    返回 session
```

这和：

```text
数据库连接池 acquire connection
浏览器自动化 create/attach browser context
Kubernetes ensure resource
```

更像。

---

# 3. 推荐：App Node = Resource Node，而不是 Command Node

建议用户看到的 Node：

```text
┌──────────────────────────┐
│ 应用                     │
│ Notepad++                │
│ 已连接 / 自动启动         │
└──────────────────────────┘
            │
            ▼
       AppSession
```

内部：

```rust
WorkflowNodeKind::Application {
    spec: ApplicationSpec,
}
```

或者如果以后统一 Operation：

```rust
Operation::AcquireApplication(...)
```

输出：

```rust
AppSessionRef
```

注意：

> Workflow JSON 里保存的是 `AppSessionRef` 的逻辑引用，不要保存 HWND、PID、COM object。

真实资源放在 Runtime 的 Resource Table 里。

---

# 4. AppSession 应该是什么

推荐把应用实例抽象成运行时资源：

```rust
pub struct AppSession {
    pub id: ResourceId,
    pub profile_id: AppProfileId,

    pub process: Option<ProcessIdentity>,
    pub windows: Vec<WindowIdentity>,

    pub capabilities: AppCapabilities,

    pub started_by_workflow: bool,
}
```

比如：

```rust
pub struct AppCapabilities {
    pub windows_uia: bool,
    pub browser_cdp: bool,
    pub visual: bool,
    pub command_adapter: bool,
}
```

这里的：

```text
uia
cdp
visual
```

不是用户配置“接下来必须用哪个”。

它是 Planner 的运行时事实。

---

# 5. App Node 推荐暴露的配置

P0 不要一次做太复杂。

我建议第一版：

```rust
pub struct ApplicationSpec {
    pub executable_path: String,
    pub arguments: Vec<String>,
    pub window_title: WindowTitleMatcher,

    pub acquire_policy: AcquirePolicy,
    pub launch_timeout_ms: u64,

    pub cleanup_policy: CleanupPolicy,
}
```

---

## 5.1 AcquirePolicy

```rust
pub enum AcquirePolicy {
    AttachOrStart,
    AttachOnly,
    AlwaysStartNew,
}
```

默认：

```text
AttachOrStart
```

对应你的数字员工场景最合理。

---

## 5.2 CleanupPolicy

```rust
pub enum CleanupPolicy {
    LeaveRunning,
    CloseIfStartedByWorkflow,
    AlwaysClose,
}
```

默认：

```text
LeaveRunning
```

或者企业 RPA 可以默认：

```text
CloseIfStartedByWorkflow
```

具体产品策略后面再定。

关键是 Session 层要有这个概念。

---

## 5.3 ActivationPolicy

以后可以加：

```rust
pub enum ActivationPolicy {
    None,
    BestEffort,
    Required,
}
```

因为：

```text
UIA semantic action
```

通常不需要真正 foreground；

而：

```text
SendInput
视觉坐标点击
```

可能需要。

因此不要把 `SetForegroundWindow` 永久写死在 App acquire 的语义里。

可以让 Planner / backend 根据动作能力判断是否需要 activation。

---

# 6. UI Action 应该怎么引用 App

当前：

```rust
AutomationTarget {
    locator,
    backend_preference,
}
```

推荐逐渐演进成：

```rust
pub struct AutomationTarget {
    pub scope: TargetScope,
    pub locator: TargetLocator,
    pub backend_preference: BackendPreference,
}
```

```rust
pub enum TargetScope {
    Current,
    AppSession(ResourceRef),
    BrowserPage(ResourceRef),
}
```

于是：

```text
Click
target.scope = appA
target.locator = button(name = "导出")
```

而不是现在：

```text
ApplicationQuery {
    ApplicationTarget {...},
    query: ...
}
```

---

# 7. `ApplicationQuery` 不需要马上删除

当前的：

```text
ApplicationQuery
```

其实非常适合保留成：

```text
方便模式 / legacy sugar
```

例如用户只做一个动作：

```text
打开记事本并输入 hello
```

没必要要求他一定画：

```text
App
 ↓
SetValue
```

内部可以继续支持：

```text
action scoped application acquire
```

但是新的 Workflow Studio 推荐优先生成：

```text
Application Node
  ↓
AppSessionRef
  ↓
UI Action
```

长期再逐渐把：

```text
ApplicationQuery
```

降级成编译期 sugar。

---

# 8. CLI / PowerShell：一定不要作为 UI Action backend

这是整个设计里我认为最需要明确的一条边界。

现在你的 BackendKind：

```text
WindowsUia
BrowserCdp
VisualCache
OcrTiny
OcrMedium
GuiGrounding
SendInput
```

它们共同回答的问题是：

> “这个 UI 操作应该怎么实现？”

例如：

```text
Click("保存")
```

可以通过：

```text
UIA InvokePattern
CDP click
OCR + SendInput
```

完成。

---

但 PowerShell 回答的是：

> “执行一段脚本/命令。”

这是另一个语义族。

所以不要变成：

```rust
enum BackendKind {
    WindowsUia,
    BrowserCdp,
    Ocr,
    PowerShell,   // 不建议
    Cli,          // 不建议
}
```

因为接下来你会遇到一个无法回答的问题：

```text
Click("保存")
```

为什么：

```text
PowerShell
```

是一个合法 backend？

它并不天然保持 Click 的语义。

---

# 9. 推荐独立 `Command` Node

用户可以看到：

```text
系统
  └─ 执行命令
```

属性：

```text
运行方式：
  直接程序
  PowerShell
  CMD
```

内部：

```rust
pub enum CommandRunner {
    Direct,
    PowerShell,
    Cmd,
}
```

推荐数据结构：

```rust
pub struct CommandOperation {
    pub runner: CommandRunner,

    // Direct 时使用
    pub program: Option<ValueExpr>,
    pub arguments: Vec<ValueExpr>,

    // PowerShell / Cmd 时使用
    pub script: Option<ValueExpr>,

    pub working_directory: Option<ValueExpr>,
    pub environment: Vec<EnvironmentBinding>,
    pub stdin: Option<ValueExpr>,

    pub timeout_ms: u64,
    pub accepted_exit_codes: Vec<i32>,
}
```

输出：

```rust
pub struct CommandOutcome {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}
```

---

# 10. 为什么 Command 可以一个 Node 内有 Direct / PS / CMD

这和前面的“不要做万能 Node”并不矛盾。

因为：

```text
Direct process
PowerShell
CMD
```

本质都属于：

```text
Command Execution
```

拥有共同的：

```text
stdin
stdout
stderr
exit code
cwd
environment
timeout
```

它们共享一个语义域。

相反：

```text
UI Click
PowerShell
OCR
```

没有共享语义域。

判断一个东西是否应该合并成一个 Node，最重要的标准不是：

> “它们是不是都能干活？”

而是：

> **“它们的输入、输出、错误、重试、生命周期和可观测性是不是同一种东西？”**

---

# 11. 你真正应该做的“通用层”在 Runtime，而不是 UI

你问：

> 要不要暴露一个拥有 CLI/PS 能力的通用 Node？

我的回答是：

> **用户层不要；Runtime 层要。**

也就是说，UI 上可以是：

```text
Application
UI Action
Read
Command
If
Loop
```

但是 Runtime 内部可以统一成：

```text
WorkflowEngine
      │
      ▼
NodeExecutorRegistry
      │
 ┌────┼───────────────┐
 ▼    ▼               ▼
App   UI Operation    Command
Exec  Executor        Executor
       │
       ▼
   ActionRouter
```

推荐：

```rust
pub trait NodeExecutor: Send + Sync {
    fn supports(&self, kind: NodeKind) -> bool;

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut RunContext,
    ) -> Result<NodeOutcome, RuntimeError>;
}
```

或者不做动态 registry，继续强类型 match 也可以。

关键不是 trait 长什么样。

关键是分层：

```text
Workflow Node Executor
        ≠
UI Backend
```

---

# 12. 最推荐的整体执行架构

```text
                         Workflow Definition
                                 │
                                 ▼
                         Workflow Validator
                                 │
                                 ▼
                         Workflow Engine
                                 │
                    ┌────────────┼────────────┐
                    │            │            │
                    ▼            ▼            ▼
              Resource Nodes   Operations   Control
                    │            │
                    │            ├──────── UI Operation
                    │            │             │
                    │            │             ▼
                    │            │        ActionRouter
                    │            │             │
                    │            │     ┌───────┼─────────┐
                    │            │     ▼       ▼         ▼
                    │            │    UIA     CDP    Vision/OCR
                    │            │
                    │            └──────── Command
                    │                       │
                    │                 ┌─────┼─────┐
                    │                 ▼     ▼     ▼
                    │               Direct  PS   CMD
                    │
                    ▼
              Runtime Resource Table
                    │
             ┌──────┼────────┐
             ▼      ▼        ▼
           App A   App B   Browser/Page
```

---

# 13. 更重要：ArgusFlow 需要 `RunContext / ExecutionState`

当前 Workflow Engine 主要维护：

```text
current node
event sequence
静态 workflow.variables
```

但数字员工必须有：

```rust
pub struct RunContext {
    pub workflow_inputs: JsonObject,

    pub node_outputs: HashMap<NodeId, NodeOutcome>,
    pub resources: ResourceTable,

    pub variables: VariableStore,

    pub cancellation: CancellationToken,
}
```

---

# 14. NodeOutcome 应该结构化

建议不要再只有：

```rust
ActionOutcome {
    backend,
    message,
}
```

而是在 Workflow Runtime 层增加：

```rust
pub struct NodeOutcome {
    pub outputs: JsonObject,
    pub resources: Vec<ResourceBinding>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
}
```

UI Action 的底层仍然可以返回：

```rust
ActionOutcome
```

然后它的 Node Executor 转换成：

```text
NodeOutcome
```

例如：

```text
Click:
outputs = {}

Read Text:
outputs = {
  "text": "ACME-10086"
}

Command:
outputs = {
  "exit_code": 0,
  "stdout": "...",
  "stderr": ""
}
```

---

# 15. 数据引用：固定流程尤其需要

不要要求用户通过一个全局 mutable JSON 到处写变量。

推荐：

```rust
pub enum ValueExpr {
    Literal(JsonValue),

    WorkflowInput {
        key: String,
    },

    NodeOutput {
        node_id: String,
        port: String,
    },

    Variable {
        name: String,
    },
}
```

`WorkflowInput` 的声明和值必须分离：声明属于持久化 schema，实际值只属于一次运行。

```rust
pub struct WorkflowInputDefinition {
    pub key: String,
    pub value_type: WorkflowInputType,
}

pub struct RunInputs {
    pub values: Map<String, Value>,
}

WorkflowEngine::start(workflow, inputs, sink)
```

`workflow.variables` 只负责初始化运行内变量，不能同时充当 `WorkflowInput`。

例如：

```text
Read Order Number
  output.text
      │
      ▼
SetValue
  value = NodeOutput(ReadOrderNumber, "text")
```

UI 上可以显示成：

```text
填写内容：
[ 读取订单号 → 文本 ]
```

而不是暴露表达式源码。

---

# 16. ResourceRef 与 ValueRef 必须分开

非常重要。

```text
"订单号 = ACME-10086"
```

属于：

```text
Value
```

而：

```text
App A 的 HWND / PID / UIA root
```

属于：

```text
Runtime Resource
```

不要把两者都塞进 JSON variable。

推荐：

```rust
pub struct ResourceRef {
    pub producer_node_id: String,
    pub output_name: String,
}
```

真实：

```text
HWND
PID
COM client
CDP session
```

只存在：

```text
ResourceTable
```

里面。

Workflow 定义只保存引用。

---

# 17. 这会让你的典型流程非常自然

用户场景：

> 打开软件 A，阅读信息；打开软件 B，填写表单记录信息。

Workflow：

```text
Start
  │
  ▼
Application: 软件 A
  │ app_a
  ▼
Read: 订单编号
  │ text
  ▼
Application: 软件 B
  │ app_b
  ▼
Set Value: 订单编号
  │
  ▼
Set Value: 客户名称
  │
  ▼
Click: 保存
  │
  ▼
End
```

逻辑关系：

```text
app_a = AppSessionRef(node_A)

order_id =
  Read(
      scope = app_a,
      locator = AQL(...)
  )

app_b = AppSessionRef(node_B)

SetValue(
    scope = app_b,
    locator = AQL(...),
    value = NodeOutputRef(order_id.text)
)
```

---

# 18. Action Node 应该继续存在，但建议分成“语义动作”

你现在：

```rust
AutomationAction {
    Click,
    SetValue,
}
```

未来可以逐步扩展：

```rust
pub enum UiOperation {
    Click,
    SetValue,
    GetValue,
    GetText,
    Select,
    Toggle,
    Expand,
    Collapse,
    Focus,
    Hotkey,
    Scroll,
    ExtractTable,
    ScreenshotRegion,
}
```

这里有两个 UX 选择。

---

## 18.1 方案 A：画布只有一个“界面操作” Node

配置里选择：

```text
点击
输入文本
读取文本
选择
...
```

优点：

- Node palette 很干净
- 内部 schema 很统一

缺点：

- 画布可读性略弱
- 用户需要点开才能知道行为

---

## 18.2 方案 B：Palette 暴露多个快捷 Node，但底层还是一个 UiOperation

例如：

```text
点击
输入文本
读取文本
选择项
```

但是保存时统一：

```rust
WorkflowNodeKind::UiOperation {
    operation: ...
}
```

我更推荐这个。

也就是：

> **UI 可以有很多“模板节点”，IR 不需要有很多互不相关的节点实现。**

这是减少用户认知成本和减少 Runtime 复杂度之间最好的平衡。

---

# 19. 不建议一个 `万能 Tool Node`

如果做：

```text
Tool Node

mode:
  ui
  ocr
  cli
  powershell
  http
  filesystem
  ...
```

短期会感觉：

```text
“只写一个 Node，扩展很快。”
```

长期会变成：

```text
ToolNodeConfig {
    mode,
    action,
    shell,
    locator,
    query,
    exe,
    cwd,
    env,
    timeout,
    output_parser,
    backend,
    screenshot,
    ...
}
```

然后出现：

- 属性面板逻辑巨大
- Validator 充满组合判断
- schema 难升级
- execution error 不统一
- retry 语义混乱
- 权限粒度混乱
- node explain 很难做
- 用户不知道节点到底在做什么

这会成为 Workflow 产品最容易失控的一块。

---

# 20. 三种方案对比

| 方案 | 简单度 | 可维护性 | 数字员工适配 | 可观测性 | 类型安全 | 推荐 |
|---|---:|---:|---:|---:|---:|---:|
| 单独 `Run App` Node | 8 | 6 | 6 | 7 | 7 | 6.5 |
| 万能 `Run/Tool` Node | 9（前期） | 2 | 5 | 3 | 2 | 3 |
| `AppSession + Semantic Operation + Planner` | 7 | 9 | 10 | 10 | 9 | **9.5** |

---

# 21. CLI / PowerShell 的安全边界建议

既然 ArgusFlow 是“数字员工”，而且会拥有 PowerShell 能力，建议从第一版就让 Command 有独立权限模型。

不要只做：

```text
script: String
```

然后任意执行。

至少预留：

```rust
pub struct WorkflowPermissions {
    pub application_launch: bool,
    pub direct_command: bool,
    pub powershell: bool,
    pub cmd: bool,
    pub filesystem_read: Vec<PathPolicy>,
    pub filesystem_write: Vec<PathPolicy>,
    pub network: bool,
}
```

`Application` 的可启动策略和三种 Command runner 必须分别检查对应能力；不能用只覆盖 Command 的 `process_spawn` 暗示全局进程边界。

Command 还应该支持：

```text
timeout
max stdout size
max stderr size
working directory
environment
accepted exit codes
```

`timeout` 必须覆盖完整命令生命周期，而不只是根进程的 `wait()`：

```text
prepare deadline
    ↓
CREATE_SUSPENDED
    ↓
assign Windows Job Object (KILL_ON_JOB_CLOSE)
    ↓
resume root thread
    ↓
stdin write + root wait + process-tree termination + stdout/stderr drain
    ↓
same deadline
```

根进程退出后必须先终止 job 中仍存活的后代，再等待管道 EOF。任何 timeout 或 I/O 失败都终止整个 job，不能只 `kill()` 直接 child；否则继承 stdout/stderr 写端的后代可以让输出任务无限等待。

当前 `WorkflowPermissions` 仍只是可信本地工作流中的能力声明。引入导入、分享、模板市场或远程工作流前，必须拆分为 workflow 自带的 `RequestedPermissions` 与宿主独立保存的 `HostGrantedPermissions`，运行时只能信任二者交集。

---

# 22. Direct Program 应该优先于 Shell String

例如不要默认：

```text
cmd /c "program.exe --foo ..."
```

更推荐：

```rust
Command::new(program)
    .args(args)
```

原因：

- 没有 shell quoting 猜测
- 少一层命令注入风险
- 参数语义明确
- Unicode 更可控
- exit code 更直接

PowerShell 只在真的需要 PowerShell 语言能力时使用。

---

# 23. App Session 与 Backend Planner 的关系

App Node 不应该选择：

```text
UIA / CDP / OCR
```

App Node 只负责：

```text
我是谁
我在哪里
我现在是否可用
我有哪些 capability
```

例如：

```text
AppSession {
    exe = chrome.exe
    hwnd = ...
    capabilities = {
        uia: true,
        cdp: true,
        visual: true
    }
}
```

然后：

```text
Click("登录")
```

Planner 看到：

```text
scope = chrome_session
AQL = button(name = "登录")
```

决定：

```text
CDP Native
```

而另一个 Electron App：

```text
CDP 不可 attach
UIA ready
```

Planner 就走：

```text
UIA
```

这正是你现有 `ExecutionContext + PreparedPlan` 架构应该继续承担的职责。

---

# 24. 不要让 Planner“聪明过头”

你的大部分目标是固定流程。

因此 Planner 应该做：

```text
选择语义等价的执行路径
```

而不是：

```text
重新决定用户下一步要干什么
```

也就是说：

```text
Workflow
```

负责 What：

```text
读取订单号
打开 CRM
填写订单号
保存
```

Planner 负责 How：

```text
UIA?
CDP?
OCR?
SendInput?
```

---

# 25. Agent / LLM 应该放在哪里

未来如果你想加入真正的“数字员工自主能力”，建议独立一个：

```text
Agent Step
```

而不是让每个 Node 都变成 Agent。

例如：

```text
Agent Step:
  goal = "从页面中找出异常订单，并逐个处理"
```

它内部可以使用：

```text
UI Action
Read
Command
AppSession
```

作为 tools。

但是：

> 对于已经明确的企业 SOP，最终最好仍然落成 deterministic workflow nodes。

原因：

- 可审计
- 可 replay
- 可调试
- 可测试
- 可做权限 review
- 性能更高
- 成本更低

---

# 26. Runtime 建议新增 `ResourceTable`

```rust
pub struct ResourceTable {
    resources: HashMap<ResourceId, ResourceEntry>,
}
```

```rust
pub enum ResourceEntry {
    Application(AppSession),
    Browser(BrowserSession),
    BrowserPage(BrowserPageSession),
}
```

资源由 Workflow Engine 管理生命周期。

---

# 27. Resource Lease / Cleanup 很值得现在就设计

App Acquire 返回：

```rust
ResourceLease<AppSession>
```

它记录：

```text
这个 App 是 Workflow 启动的？
还是本来就存在？
```

Workflow 结束时：

```text
cleanup_policy = LeaveRunning
    → 什么也不做

cleanup_policy = CloseIfStartedByWorkflow
    → 仅关闭自己启动的

cleanup_policy = AlwaysClose
    → 尝试关闭
```

这能避免企业 RPA 非常常见的问题：

```text
一次 workflow run 留下十几个孤儿进程
```

---

# 28. App Node 要天然幂等

这是固定流程 + retry 的关键。

比如：

```text
Application Node
    ↓
Read
```

Read 因网络卡住失败，整个段落 retry。

如果 Application Node 是：

```text
Run = CreateProcess()
```

retry 就会：

```text
再开一个 App
```

如果语义是：

```text
Ensure App Session
```

retry 就是：

```text
已有 session 仍健康？
    yes → reuse
    no  → reacquire
```

这才适合 Workflow runtime。

---

# 29. 建议引入 Node Retry Policy，但不要每种 Node 自己乱做

以后：

```rust
pub struct NodeExecutionPolicy {
    pub timeout_ms: Option<u64>,
    pub retry: RetryPolicy,
}
```

可以作为所有 executable node 的 common metadata。

但 Node / backend 应该返回结构化错误分类：

```text
Retryable
NonRetryable
ContextLost
TargetNotFound
PermissionDenied
ProcessExited
```

让 Runtime 决定是否 retry。

---

# 30. AppSession 失效怎么处理

数字员工一定会遇到：

```text
App crash
窗口重建
Browser tab reload
PID 改变
Electron renderer 重启
```

所以 ResourceRef 不能等同于：

```text
固定 HWND
```

而应该是一个：

```text
Logical Session
```

内部可以：

```text
resolve current window
refresh target
reconnect cdp
```

但是必须有边界：

- 如果只是窗口 HWND 重建，可以恢复
- 如果业务应用完全退出，根据 session policy 决定 restart
- 如果出现两个同名窗口，必须进入 ambiguous error，而不是随便选一个

你当前 UIA 对多窗口要求唯一匹配的做法应该保留。

---

# 31. App Profile 与 App Session 最好分开

未来建议：

```text
AppProfile = 持久化配置
AppSession = 一次 Run 中的运行资源
```

例如：

```rust
pub struct AppProfile {
    pub id: AppProfileId,
    pub name: String,
    pub executable_path: String,
    pub launch_args: Vec<String>,
    pub window_matcher: WindowTitleMatcher,
}
```

Workflow Node 可以：

```text
应用：
  Profile = ERP
```

而不是每个 workflow 重复写：

```text
C:\Program Files\ERP\erp.exe
```

这对企业数字员工会很重要。

---

# 32. 浏览器怎么处理

短期可以让：

```text
Application Session
```

覆盖 Chrome / Edge 顶层应用。

然后 Runtime 发现：

```text
CDP session available
```

ActionRouter 自动使用 CDP。

长期建议增加子资源：

```text
BrowserSession
BrowserPageSession
```

因为：

```text
一个浏览器进程
```

和：

```text
一个具体 Tab / target
```

不是同一个资源。

最终可能是：

```text
Chrome AppSession
     │
     ▼
BrowserSession
     │
     ▼
PageSession
```

但第一版不用一次做完。

---

# 33. `Read` 是必须补的一等 Action

你的目标场景里：

```text
阅读信息
```

是核心能力。

现在 `AutomationAction` 只有：

```text
Click
SetValue
```

建议下一批优先加：

```text
GetText
GetValue
GetAttribute
```

而不是先加几十个控制节点。

因为它直接解锁：

```text
App A → App B
```

的数据传递。

---

# 34. Read 的 backend 也应该走 Planner

例如：

```text
GetText(button...)
```

可能走：

```text
UIA Name
CDP textContent
OCR
```

这和 Click 一样，本质是：

```text
同语义，不同 backend
```

所以应该进入现有 ActionRouter 的体系。

---

# 35. Command Node 不走 ActionRouter，但可以复用 PreparedPlan 思想

虽然 Command 不应该成为 UI Backend，但你现有 PreparedPlan 的设计思想可以复用。

例如：

```text
Command Operation
    ↓
prepare
    ↓
resolve ValueExpr
validate program
validate permission
freeze cwd/env
    ↓
PreparedCommand
    ↓
execute
```

这样：

```text
Explain 的东西
```

和：

```text
真正执行的东西
```

仍然一致。

---

# 36. 我建议把“准备”和“执行”推广成整个 Runtime 的原则

现在你已经在 UI Backend 里这么做了：

```text
prepare → PreparedPlan → execute
```

未来可推广：

```text
Workflow Node
    ↓
prepare
    ↓
PreparedNodeExecution
    ↓
execute
```

例如：

```text
App:
  resolve profile
  check executable
  freeze acquire policy
  → PreparedAppAcquire

Command:
  resolve arguments
  permission check
  → PreparedCommand

UI Action:
  resolve AppSession
  → ActionRouter
  → PreparedPlan
```

---

# 37. Node Schema 推荐方向

不建议把所有东西塞进：

```rust
WorkflowNodeKind::Action
```

长期可以变成：

```rust
pub enum WorkflowNodeKind {
    Start,
    End,

    Log { ... },
    Delay { ... },
    Condition { ... },

    Application {
        spec: ApplicationSpec,
    },

    Ui {
        operation: UiOperation,
    },

    Command {
        operation: CommandOperation,
    },
}
```

这是最容易从你当前代码渐进演进的版本。

---

# 38. 更通用的内部 IR 可以再晚一点做

如果未来 Node 类型很多，可以内部变成：

```rust
WorkflowNodeKind::Operation {
    operation: Operation,
}
```

```rust
pub enum Operation {
    Application(ApplicationOperation),
    Ui(UiOperation),
    Command(CommandOperation),
    Data(DataOperation),
    File(FileOperation),
}
```

但是我不建议你现在就为了“抽象漂亮”做这个重构。

先把：

```text
Application
Ui
Command
NodeOutput
ResourceRef
```

跑通。

---

# 39. Validator 需要新增一条非常重要的能力：Resource Dominance

例如：

```text
       If
      /  \
   AppA   Nothing
      \  /
      Click(scope = AppA)
```

那么：

```text
Click
```

不能保证一定能拿到：

```text
AppA session
```

Validator 应该能提前报错。

本质上需要验证：

> 产生 Resource 的 Node 必须在所有能够到达 Consumer 的控制路径上先执行。

也就是 CFG 上的：

```text
dominator
```

关系。

对于固定 Workflow，这个静态校验非常值钱。

---

# 40. ValueRef 也可以做同样校验

例如：

```text
       If
      /  \
   Read   Skip
      \  /
     SetValue(read.text)
```

同样存在：

```text
read.text
```

可能不存在。

因此：

```text
ResourceRef
ValueRef
```

都应该进入 Workflow Validator。

---

# 41. 为什么这会让 ArgusFlow 比传统 RPA 更强

很多传统 RPA 最后会变成：

```text
一大堆 UI 操作 block
+ 全局变量
+ selector
```

你现在有机会把底层做成：

```text
Typed Workflow
+
Typed Resource
+
Typed Data Ref
+
Query Compiler
+
Prepared Planner
```

这其实更像：

```text
一个针对 Desktop / Browser Automation 的编译型 Workflow Runtime
```

而不仅仅是 RPA block editor。

---

# 42. 产品层 Node Palette，我建议最终长这样

```text
流程
├─ 开始
├─ 结束
├─ 条件
├─ 循环
├─ 等待
├─ 等待直到
├─ 重试
└─ 子流程

应用
├─ 打开或连接应用
└─ 浏览器页面（后续）

界面
├─ 点击
├─ 输入文本
├─ 读取文本
├─ 选择
├─ 快捷键
└─ 提取表格

系统
├─ 执行命令
└─ 文件操作（后续）

数据
├─ 转换
├─ JSON
├─ 文本
└─ 表格（后续）
```

注意：

```text
UIA
CDP
OCR
PowerShell
```

都不需要作为一级 Node 类别。

---

# 43. 高级用户怎么强制 UIA / CDP

你当前：

```rust
BackendPreference {
    Auto,
    WindowsUia,
    BrowserCdp,
}
```

可以继续保留。

但 UI 上放在：

```text
高级设置
  执行引擎：
    自动（推荐）
    Windows UIA
    Browser CDP
```

而不是让用户从 Palette 拖：

```text
UIA Click
CDP Click
OCR Click
```

---

# 44. OCR 也不应该单独成为业务 Node

用户不应该写：

```text
OCR “保存”
  ↓
点击坐标
```

除非是在做真正的 OCR 数据处理。

对于：

```text
点击“保存”
```

OCR 是定位 fallback。

所以：

```text
VisualCache / OcrTiny / OcrMedium / Grounding
```

继续留在 Planner。

这与你现在的设计完全一致。

---

# 45. 哪些东西可以成为独立 Node

判断标准：

> 它是否拥有不同的业务输入/输出/错误/生命周期？

适合独立 Node：

```text
Application Session
Command
HTTP Request
File
Database
Human Approval
Subflow
```

不适合独立业务 Node：

```text
UIA
CDP
OCR Tiny
OCR Medium
SendInput
```

因为后者只是执行策略。

---

# 46. 建议的 RunContext 草案

```rust
pub struct RunContext {
    pub run_id: Uuid,

    pub inputs: JsonObject,

    pub node_outputs: HashMap<String, NodeOutputMap>,

    pub resources: ResourceTable,

    pub variables: VariableStore,

    pub execution_context_provider:
        Arc<dyn ExecutionContextProvider>,
}
```

---

# 47. Node 输出接口草案

```rust
pub type NodeOutputMap =
    BTreeMap<String, RuntimeValue>;
```

```rust
pub enum RuntimeValue {
    Json(JsonValue),

    // 只用于 Runtime 内部，序列化时不能泄露真实 OS handle
    Resource(ResourceRef),
}
```

甚至更严格：

```text
Value outputs
Resource outputs
```

完全分两个 map。

---

# 48. Event 也建议支持结构化 Payload

当前事件更多是：

```text
kind
message
```

以后建议：

```rust
ExecutionEvent {
    ...
    message: Option<String>,
    payload: Option<ExecutionEventPayload>,
}
```

例如：

```text
NodeOutputProduced
ResourceAcquired
BackendSelected
RetryScheduled
CommandExited
```

开发者模式会非常好用。

---

# 49. 但注意不要把 stdout / UI 文本全部无脑写日志

数字员工以后会处理：

```text
客户数据
账号
内部系统信息
```

所以 Event / telemetry 应该支持：

```text
redacted
sensitive
preview only
```

避免 Runtime debug log 自动泄露完整业务数据。

---

# 50. 建议的实现顺序

我会按这个顺序做，而不是先造一个万能 Run Node。

## Phase 1：补 Runtime 数据面

新增：

```text
RunContext
NodeOutcome
ValueExpr / NodeOutputRef
```

先让：

```text
Read → SetValue
```

跑起来。

这是数字员工最基础的一条链。

---

## Phase 2：App Resource

新增：

```text
Application Node
AppSession
ResourceTable
ResourceRef
```

把当前：

```text
WindowService::resolve_application()
```

从 UIA action-scoped 能力提升成可复用 Resource 能力。

---

## Phase 3：AutomationTarget 加 Scope

从：

```text
ApplicationQuery
```

逐渐迁移到：

```text
scope = AppSessionRef
locator = Query
```

保留 legacy ApplicationQuery。

---

## Phase 4：Command Node

新增：

```text
Direct
PowerShell
CMD
```

统一：

```text
stdout
stderr
exit code
timeout
```

不要放进 ActionRouter。

---

## Phase 5：扩展 UI Operation

优先：

```text
GetText
GetValue
Select
Hotkey
```

然后再考虑复杂表格、drag/drop 等。

---

## Phase 6：控制流增强

固定流程真正需要的：

```text
Loop / ForEach
WaitUntil
Retry
Switch
Subflow
```

优先级通常比：

```text
Parallel
Agent
```

更高。

---

# 51. 对当前代码的最小迁移路线

现在：

```rust
WorkflowNodeKind::Action {
    action: AutomationAction
}
```

第一刀可以只新增：

```rust
WorkflowNodeKind::Application {
    application: ApplicationSpec
}
```

和：

```rust
WorkflowNodeKind::Command {
    command: CommandOperation
}
```

然后给 Engine 增加：

```text
RunContext
```

---

# 52. 第一版 Application Node 可以复用现在 WindowService

你已经有：

```text
validate exe
find existing window
require unique window
spawn
wait
restore
best effort foreground
```

所以不用重新写。

只需要把：

```text
resolve_application()
```

的结果从：

```text
UiaPreparedExecution 内部临时使用
```

提升成：

```text
ResourceTable 中的 AppSession
```

---

# 53. 第一版甚至不需要马上支持复杂 App 类型

你现在的 `ApplicationTarget` 明确只支持：

```text
direct-process Windows desktop app
```

这个约束其实非常好。

P0 可以继续：

```text
绝对 EXE path
arguments
window title
timeout
```

不要为了“任何 App”第一版就做：

```text
UWP
AUMID
bootstrapper
singleton relay
services
multi-process app graph
```

先把 Resource 模型跑稳。

---

# 54. 后续再扩 Application Resolver

未来可以把：

```rust
ApplicationIdentity
```

扩成：

```rust
pub enum ApplicationIdentity {
    ExecutablePath(PathBuf),
    ProcessName(String),
    AppUserModelId(String),
    CustomProfile(AppProfileId),
}
```

启动方式：

```rust
pub enum LaunchSpec {
    Executable { path, args },
    ShellExecute { target },
    PowerShell { ... },
}
```

但 `App Session` 的上层契约不用跟着推翻。

---

# 55. 一个重要区别：Raw CLI 与 App-specific Adapter

以后可能出现：

```text
Excel
```

既可以 UIA 操作，也可以：

```text
COM / Native API
```

或者某个 App 有官方 CLI。

这时候：

```text
App-specific Native Adapter
```

可以成为 Semantic Operation 的 backend。

例如：

```text
ReadCell(A1)
```

可能：

```text
Excel COM
UIA
```

都能保持语义。

这和：

```text
任意 PowerShell script
```

完全不同。

所以未来你可以允许：

```text
NativeApi backend
AppAdapter backend
```

进入 Planner。

但不要允许：

```text
RawShell backend
```

进入所有 UI Action 的 fallback 列表。

---

# 56. 最终建议的“聪明之处”不是万能 Node，而是 Capability-driven Planner

你真正可以做得比传统 RPA 聪明的地方是：

```text
用户只描述语义
        ↓
Compiler 理解目标
        ↓
AppSession 提供运行上下文
        ↓
Planner 选择最快、最稳执行路径
```

例如：

```text
读取“订单号”
```

Planner：

```text
CDP textContent     1ms
UIA cached value    3ms
VisualCache         5ms
OCR tiny           15ms
OCR medium         40ms
Grounding          500ms
```

然后自动选择。

这比：

```text
一个 Node 里塞 20 个 mode
```

聪明得多。

---

# 57. 我给 ArgusFlow 的最终 Node 哲学

一句话：

> **Node 表达“业务语义或资源生命周期”，Backend 表达“执行方式”。**

因此：

```text
打开/连接应用
读取文本
填写文本
执行命令
条件
循环
```

是 Node。

而：

```text
UIA
CDP
OCR
SendInput
PowerShell runtime implementation
```

通常不是业务 Node。

PowerShell 属于 `Command` Node 的 runner。

---

# 58. 推荐命名

我不建议：

```text
App Run
```

推荐用户层：

```text
应用
打开或连接应用
```

英文：

```text
Application
Open or Attach App
```

内部：

```text
AcquireApplication
AppSession
```

---

# 59. 推荐画布 UX：资源线可以“显式存储，隐式显示”

如果每个 UI Action 都画：

```text
AppSession → Click
AppSession → Read
AppSession → SetValue
```

画布可能很乱。

我建议：

## 底层

必须显式：

```text
scope = ResourceRef(appA)
```

## UI

默认可以自动绑定：

```text
最近一个可支配（dominating）的 Application Node
```

节点卡片显示：

```text
应用：ERP
```

而不是额外画 Resource Edge。

只有高级模式才显示资源关系。

这是一个很值得做的 UX。

---

# 60. 更进一步：App Scope Group

以后可以支持：

```text
┌──────────── ERP ────────────┐
│ Read Customer               │
│ Click Orders                │
│ Extract Table               │
└─────────────────────────────┘
```

视觉上像一个 Scope。

内部仍然保存：

```text
ResourceRef
```

不要真的依赖 DOM nesting 来决定运行语义。

---

# 61. 推荐 Schema v5 的方向

示意，不建议现在照抄字段名：

```json
{
  "schema_version": 5,
  "nodes": [
    {
      "id": "app-a",
      "type": "application",
      "application": {
        "profile": "erp",
        "acquire_policy": "attach_or_start",
        "cleanup_policy": "close_if_started"
      }
    },
    {
      "id": "read-order",
      "type": "ui",
      "operation": {
        "type": "get_text",
        "target": {
          "scope": {
            "type": "resource",
            "node_id": "app-a",
            "output": "session"
          },
          "locator": {
            "type": "query",
            "query": {
              "language_version": 1,
              "source": "textbox(name = \"订单号\")"
            }
          },
          "backend_preference": "auto"
        }
      }
    },
    {
      "id": "app-b",
      "type": "application",
      "application": {
        "profile": "crm",
        "acquire_policy": "attach_or_start"
      }
    },
    {
      "id": "write-order",
      "type": "ui",
      "operation": {
        "type": "set_value",
        "target": {
          "scope": {
            "type": "resource",
            "node_id": "app-b",
            "output": "session"
          },
          "locator": {
            "type": "query",
            "query": {
              "language_version": 1,
              "source": "textbox(name = \"订单编号\")"
            }
          }
        },
        "value": {
          "type": "node_output",
          "node_id": "read-order",
          "output": "text"
        }
      }
    }
  ]
}
```

这就已经非常接近真正的数字员工 Workflow IR。

---

# 62. Rust Runtime 结构建议

```text
crates/
├─ argusflow-core/
│  ├─ workflow/
│  ├─ value/
│  ├─ resource/
│  ├─ operation/
│  └─ automation/
│
├─ argusflow-runtime/
│  ├─ engine.rs
│  ├─ run_context.rs
│  ├─ resource_table.rs
│  ├─ value_resolver.rs
│  ├─ node_executor.rs
│  └─ validator/
│
├─ argusflow-agent/
│  └─ UI semantic action planner
│
├─ argusflow-windows/
│  ├─ application/
│  ├─ window/
│  ├─ uia/
│  └─ input/
│
├─ argusflow-browser/
│  └─ cdp/
│
└─ argusflow-command/
   ├─ direct.rs
   ├─ powershell.rs
   └─ cmd.rs
```

`argusflow-command` 是否独立 crate 可以等代码量上来再拆。

---

# 63. 不要把 Application 生命周期放进 UIA 专属模块

当前 `WindowService` 在：

```text
argusflow-windows
```

是合理的。

但长期：

```text
AcquireApplication
```

应该是 Windows / Runtime Resource 层能力。

因为：

```text
CDP
Visual
SendInput
```

也都可能需要同一个 AppSession。

也就是说：

```text
Application Session
```

不应该属于：

```text
UIA backend
```

它应该属于：

```text
Windows application runtime
```

UIA 只是消费它。

---

# 64. ExecutionContext 也应该可以从 AppSession 派生

现在 `ExecutionContext` 偏全局：

```text
foreground_window
active_process
browser_session
accessibility
visual_cache
```

未来 UI Action 有：

```text
scope = AppSession
```

以后 Planner 可以构造：

```text
ScopedExecutionContext
```

例如：

```rust
pub struct ScopedExecutionContext {
    pub global: ExecutionContext,
    pub application: Option<AppSessionSnapshot>,
    pub browser_page: Option<BrowserPageSnapshot>,
}
```

这样 Planner 不再过度依赖：

```text
当前 foreground
```

对于多 App Workflow 会更稳。

---

# 65. 这是你从 RPA 走向“数字员工 Runtime”的关键一步

传统简单 RPA：

```text
鼠标
键盘
selector
```

ArgusFlow 可以变成：

```text
Workflow Compiler
    │
    ├─ Control Flow
    ├─ Data Flow
    ├─ Resource Lifetime
    └─ Semantic UI Operations
            │
            ▼
       Backend Planner
```

这才是长期真正有壁垒的地方。

---

# 66. 最后给一个明确决策

如果今天让我直接在你的 repo 里继续做，我会这样定：

### 不做

```text
❌ Universal Run Node
❌ UIA Node / CDP Node / OCR Node
❌ PowerShell 作为 UI Backend
❌ 每个 UI Action 自己重复启动 App 作为长期模型
```

### 做

```text
✅ Application Resource Node
✅ AppSession / ResourceRef
✅ RunContext / ResourceTable
✅ NodeOutput / ValueRef
✅ UI Semantic Action
✅ Command Node（Direct / PowerShell / CMD）
✅ 继续使用 ActionRouter / PreparedPlan 选择 UIA/CDP/OCR
```

---

# 67. 我认为下一步最值得开的 5 个开发任务

按优先级：

```text
P0
1. Runtime 增加 RunContext + NodeOutcome + ValueRef
2. AutomationAction 增加 GetText/GetValue
3. Application Node + AppSession + ResourceTable

P1
4. AutomationTarget 增加 TargetScope(ResourceRef)
5. Command Node（Direct / PowerShell / CMD）
```

然后：

```text
P2
Loop / ForEach
WaitUntil
Retry policy
App Profile
Resource dominance validation
```

---

# 68. 最短版本

你问：

> App run 应该暴露一个 run node，还是暴露 CLI/PS 的通用 node，还是更加聪明？

我的答案：

> **更加聪明的方式是：App 不作为“命令”，而作为“资源 / Session”；UIA/CDP/OCR 作为语义 UI Action 的后端；CLI/PowerShell 作为独立 Command Operation。Workflow Runtime 再增加 Node Output + ResourceRef，把这些东西真正连起来。**

最推荐的核心模型只有四个词：

```text
Resource
Operation
Value
Control
```

ArgusFlow 的现有：

```text
AQL
PreparedPlan
ActionRouter
ExecutionContext
```

已经把 `Operation → Backend` 这半边做得很好。

下一阶段最应该补的就是：

```text
Workflow
  ↓
Resource + Value
```

这会比新增任何一个万能 Node 都更值。

---

# 参考的当前仓库文件

本方案重点参考了当前 `main` 中：

```text
docs/架构.md
docs/方案.md
docs/优化方案.md
docs/ArgusFlow_真实_UIA_对接方案_NotepadPP_E2E.md

crates/argusflow-core/src/workflow.rs
crates/argusflow-core/src/automation.rs

crates/argusflow-runtime/src/engine.rs
crates/argusflow-runtime/src/dispatcher.rs

crates/argusflow-agent/src/backend.rs
crates/argusflow-agent/src/context.rs
crates/argusflow-agent/src/plan.rs
crates/argusflow-agent/src/router.rs

crates/argusflow-windows/src/uia/backend.rs
crates/argusflow-windows/src/window/application.rs

src/features/workflow/contracts.ts
src/features/workflow/workflowModel.ts
```

其中对本方案影响最大的现状是：

```text
1. Workflow 目前只有 Start/Log/Delay/Condition/Action/End。
2. AutomationAction 目前只有 Click/SetValue。
3. ApplicationQuery 已能隐式 ensure/launch/restore app。
4. ActionRouter 已有 PreparedPlan + ExecutionContext。
5. Workflow variables 当前是只读 JSON。
6. ActionOutcome 当前没有结构化业务输出。
```

因此建议不是推翻现有设计，而是沿着现有边界继续向上补：

```text
Data Plane
+
Resource Plane
```

然后让已经存在的：

```text
Semantic Action Planner
```

继续发挥作用。
